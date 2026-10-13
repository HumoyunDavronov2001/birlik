//! The cluster's agreed state (`protocol/CLUSTER.md` §3.2): a log replicated with Raft (openraft) among the computers
//! of a cluster, at most five of them voting, so the cluster agrees on who its members are, which tokens are revoked,
//! its name and its relays — and from stage B3 on its placement plans.
//!
//! The device that creates a cluster starts the log alone. A computer it admits joins as a learner and becomes a
//! voter once it keeps up, until five vote; phones and browsers never vote and follow the state through gossip. A
//! change is proposed on any voter and forwarded to the leader ([`Consensus::propose`]).

pub mod network;
pub mod storage;

use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use omnarc_network::{Endpoint, EndpointId};
use openraft::{BasicNode, ChangeMembers, Config, ServerState};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use crate::ring::BoxError;

openraft::declare_raft_types!(
    /// The types of the cluster's Raft.
    pub TypeConfig:
        D = Command,
        R = (),
        NodeId = u64,
        Node = BasicNode,
        Entry = openraft::Entry<TypeConfig>,
        SnapshotData = Cursor<Vec<u8>>,
        AsyncRuntime = openraft::TokioRuntime,
);

pub type Raft = openraft::Raft<TypeConfig>;

/// Voters at most; more computers follow as learners.
pub const MAX_VOTERS: usize = 5;

/// A change to the agreed state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    /// A device was admitted with this membership token.
    AddMember { id: String, token: Vec<u8> },
    /// A member was removed: it is let in no more, and these revocation ids of its tokens are refused.
    RemoveMember { id: String, revocation_ids: Vec<String> },
    /// The cluster's name.
    SetName(String),
    /// The cluster's relays, with their tokens.
    SetRelays(Vec<String>),
}

/// What the cluster agreed on.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Agreed {
    pub name: String,
    /// Members by id (hex), with their membership tokens.
    pub members: BTreeMap<String, Vec<u8>>,
    /// Removed members (hex ids): let in no more.
    #[serde(default)]
    pub removed: BTreeSet<String>,
    /// Revocation ids (hex) of the tokens of removed members.
    pub revoked: BTreeSet<String>,
    pub relays: Vec<String>,
}

impl Agreed {
    fn apply(&mut self, command: &Command) {
        match command {
            Command::AddMember { id, token } => {
                // a removed member comes back only with a new token, through a new invitation
                self.removed.remove(id);
                self.members.insert(id.clone(), token.clone());
            }
            Command::RemoveMember { id, revocation_ids } => {
                self.members.remove(id);
                self.removed.insert(id.clone());
                self.revoked.extend(revocation_ids.iter().cloned());
            }
            Command::SetName(name) => name.clone_into(&mut self.name),
            Command::SetRelays(relays) => relays.clone_into(&mut self.relays),
        }
    }
}

/// The Raft id of a node: the first eight bytes of its key. Collisions among five voters are out of the question.
#[must_use]
pub fn raft_id(id: EndpointId) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&id.as_bytes()[..8]);
    u64::from_le_bytes(bytes)
}

/// The node record of a member in the Raft membership: its id, by which it is dialed.
#[must_use]
pub fn raft_node(id: EndpointId) -> BasicNode {
    BasicNode { addr: id.to_string() }
}

/// This node's part in the cluster's Raft.
#[derive(Clone)]
pub struct Consensus {
    raft: Raft,
    me: EndpointId,
    network: network::Network,
    agreed: watch::Receiver<Agreed>,
}

impl std::fmt::Debug for Consensus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Consensus").field("me", &self.me.fmt_short().to_string()).finish_non_exhaustive()
    }
}

impl Consensus {
    /// Starts this node's Raft with its log in `dir`. It answers on [`network::RAFT_ALPN`] through the returned
    /// handler, which the node's router accepts behind its members-only guard.
    ///
    /// # Errors
    /// The log cannot be opened.
    pub async fn start(dir: &Path, endpoint: Endpoint) -> Result<(Self, network::RaftHandler), BoxError> {
        let me = endpoint.id();
        let (log, machine, agreed) = storage::open(&dir.join("raft.redb"))?;
        let config = Config {
            cluster_name: "omnarc".into(),
            // links reach across networks: a relay adds tens of milliseconds
            heartbeat_interval: 300,
            election_timeout_min: 1500,
            election_timeout_max: 3000,
            ..Config::default()
        };
        let config = Arc::new(config.validate()?);
        let network = network::Network::new(endpoint);
        let raft = Raft::new(raft_id(me), config, network.clone(), log, machine).await?;
        let handler = network::RaftHandler::new(raft.clone());
        Ok((Self { raft, me, network, agreed }, handler))
    }

    /// Starts a log with this node as its only voter, unless it has one already.
    ///
    /// # Errors
    /// Raft has stopped.
    pub async fn initialize_alone(&self) -> Result<(), BoxError> {
        if self.raft.is_initialized().await? {
            return Ok(());
        }
        let voters = BTreeMap::from([(raft_id(self.me), raft_node(self.me))]);
        self.raft.initialize(voters).await?;
        Ok(())
    }

    /// The agreed state, as it changes.
    #[must_use]
    pub fn agreed(&self) -> watch::Receiver<Agreed> {
        self.agreed.clone()
    }

    #[must_use]
    pub fn raft(&self) -> &Raft {
        &self.raft
    }

    /// Whether this node leads the log now.
    #[must_use]
    pub fn is_leader(&self) -> bool {
        self.raft.metrics().borrow().state == ServerState::Leader
    }

    /// Commits `command` on the leader, wherever it is, and waits until it is committed.
    ///
    /// # Errors
    /// No leader is known, it cannot be reached, or Raft has stopped.
    pub async fn propose(&self, command: Command) -> Result<(), BoxError> {
        match self.raft.client_write(command.clone()).await {
            Ok(_) => Ok(()),
            Err(openraft::error::RaftError::APIError(openraft::error::ClientWriteError::ForwardToLeader(forward))) => {
                let leader = forward.leader_node.ok_or("no leader is known yet")?;
                self.network.forward(&leader, command).await
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Adds `member` as a learner, so it receives the log; [`Consensus::promote`] makes it a voter once it keeps up.
    /// Only the leader adds learners; elsewhere this does nothing.
    ///
    /// # Errors
    /// Raft has stopped.
    pub async fn add_learner(&self, member: EndpointId) -> Result<(), BoxError> {
        if !self.is_leader() {
            return Ok(());
        }
        match self.raft.add_learner(raft_id(member), raft_node(member), false).await {
            Ok(_) | Err(openraft::error::RaftError::APIError(_)) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// On the leader: makes learners that keep up with the log voters, until [`MAX_VOTERS`] vote.
    ///
    /// # Errors
    /// Raft has stopped.
    pub async fn promote(&self) -> Result<(), BoxError> {
        let metrics = self.raft.metrics().borrow().clone();
        if metrics.state != ServerState::Leader {
            return Ok(());
        }
        let membership = metrics.membership_config.membership().clone();
        let voters: BTreeSet<u64> = membership.voter_ids().collect();
        let Some(last) = metrics.last_log_index else { return Ok(()) };
        let matched = metrics.replication.unwrap_or_default();
        let ready: BTreeSet<u64> = membership
            .learner_ids()
            .filter(|id| matched.get(id).copied().flatten().is_some_and(|log_id| log_id.index + 1 >= last))
            .take(MAX_VOTERS.saturating_sub(voters.len()))
            .collect();
        if ready.is_empty() {
            return Ok(());
        }
        match self.raft.change_membership(ChangeMembers::AddVoterIds(ready), true).await {
            Ok(_) | Err(openraft::error::RaftError::APIError(_)) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Stops this node's Raft.
    pub async fn shutdown(&self) {
        let _ = tokio::time::timeout(Duration::from_secs(2), self.raft.shutdown()).await;
    }
}
