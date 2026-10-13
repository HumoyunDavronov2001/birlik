//! Raft between members on the ALPN `omnarc/raft/1`: one short bidirectional stream per call, a request and its
//! answer as length-prefixed JSON. The node of a Raft member is its key, so it is dialed like any member, directly or
//! through the cluster's relay; only members are let in.

use std::sync::Arc;
use std::time::Duration;

use omnarc_network::protocol::{AcceptError, ProtocolHandler};
use omnarc_network::{Connection, Endpoint, EndpointId, RecvStream, SendStream};
use openraft::BasicNode;
use openraft::error::{InstallSnapshotError, NetworkError, RPCError, RaftError, RemoteError, Unreachable};
use openraft::network::{RPCOption, RaftNetwork, RaftNetworkFactory};
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse, VoteRequest,
    VoteResponse,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use super::{Command, Raft, TypeConfig};
use crate::ring::BoxError;

/// ALPN of Raft between members.
pub const RAFT_ALPN: &[u8] = b"omnarc/raft/1";

/// The largest message: snapshots go in chunks far smaller.
const MAX_MESSAGE: usize = 16 << 20;

/// How long a call waits for its answer.
const CALL_TIMEOUT: Duration = Duration::from_secs(5);

type NodeId = u64;

#[derive(Debug, Serialize, Deserialize)]
enum Call {
    Append(AppendEntriesRequest<TypeConfig>),
    Vote(VoteRequest<NodeId>),
    Snapshot(InstallSnapshotRequest<TypeConfig>),
    /// A change proposed on a follower, for the leader to commit.
    Write(Command),
}

#[derive(Debug, Serialize, Deserialize)]
enum Answer {
    Append(Result<AppendEntriesResponse<NodeId>, RaftError<NodeId>>),
    Vote(Result<VoteResponse<NodeId>, RaftError<NodeId>>),
    Snapshot(Result<InstallSnapshotResponse<NodeId>, RaftError<NodeId, InstallSnapshotError>>),
    Write(Result<(), String>),
}

async fn write_message<T: Serialize>(send: &mut SendStream, message: &T) -> Result<(), BoxError> {
    let body = serde_json::to_vec(message)?;
    let len = u32::try_from(body.len())?;
    send.write_all(&len.to_le_bytes()).await?;
    send.write_all(&body).await?;
    send.finish()?;
    Ok(())
}

async fn read_message<T: for<'de> Deserialize<'de>>(recv: &mut RecvStream) -> Result<T, BoxError> {
    let mut len = [0u8; 4];
    recv.read_exact(&mut len).await?;
    let len = usize::try_from(u32::from_le_bytes(len))?;
    if len > MAX_MESSAGE {
        return Err(format!("a Raft message of {len} bytes").into());
    }
    let mut body = vec![0u8; len];
    recv.read_exact(&mut body).await?;
    Ok(serde_json::from_slice(&body)?)
}

/// Answers the Raft calls of other members.
#[derive(Clone)]
pub struct RaftHandler {
    raft: Raft,
}

impl std::fmt::Debug for RaftHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RaftHandler")
    }
}

impl RaftHandler {
    pub(super) fn new(raft: Raft) -> Self {
        Self { raft }
    }

    async fn answer(&self, mut send: SendStream, mut recv: RecvStream) -> Result<(), BoxError> {
        let call: Call = read_message(&mut recv).await?;
        let answer = match call {
            Call::Append(request) => Answer::Append(self.raft.append_entries(request).await),
            Call::Vote(request) => Answer::Vote(self.raft.vote(request).await),
            Call::Snapshot(request) => Answer::Snapshot(self.raft.install_snapshot(request).await),
            Call::Write(command) => {
                Answer::Write(self.raft.client_write(command).await.map(|_| ()).map_err(|e| e.to_string()))
            }
        };
        write_message(&mut send, &answer).await
    }
}

impl ProtocolHandler for RaftHandler {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        while let Ok((send, recv)) = connection.accept_bi().await {
            let handler = self.clone();
            tokio::spawn(async move {
                let _ = handler.answer(send, recv).await;
            });
        }
        Ok(())
    }
}

/// Makes the clients through which this node's Raft calls the others.
#[derive(Debug, Clone)]
pub struct Network {
    endpoint: Endpoint,
}

impl Network {
    pub(super) fn new(endpoint: Endpoint) -> Self {
        Self { endpoint }
    }

    /// Hands `command` to the leader `leader` to commit.
    pub(super) async fn forward(&self, leader: &BasicNode, command: Command) -> Result<(), BoxError> {
        let mut client = Client::new(self.endpoint.clone(), 0, leader);
        match client.call(Call::Write(command)).await.map_err(|e| e.to_string())? {
            Answer::Write(result) => result.map_err(Into::into),
            _ => Err("the leader answered something else".into()),
        }
    }
}

impl RaftNetworkFactory<TypeConfig> for Network {
    type Network = Client;

    // the trait is async; a client connects on its first call
    #[allow(clippy::unused_async_trait_impl)]
    async fn new_client(&mut self, target: NodeId, node: &BasicNode) -> Client {
        Client::new(self.endpoint.clone(), target, node)
    }
}

/// Calls one other member; its connection is opened on the first call and again after it breaks.
#[derive(Debug)]
pub struct Client {
    endpoint: Endpoint,
    target: NodeId,
    peer: Option<EndpointId>,
    connection: Arc<Mutex<Option<Connection>>>,
}

#[derive(Debug)]
struct CallError(String);

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CallError {}

impl Client {
    fn new(endpoint: Endpoint, target: NodeId, node: &BasicNode) -> Self {
        Self { endpoint, target, peer: node.addr.parse().ok(), connection: Arc::new(Mutex::new(None)) }
    }

    /// Sends `call` and waits for its answer. A peer that cannot be dialed is unreachable (Raft backs off); a
    /// broken stream is a network error (Raft retries).
    async fn call(&mut self, call: Call) -> Result<Answer, Failure> {
        let peer = self.peer.ok_or_else(|| Failure::Unreachable(CallError("a member without an id".into())))?;
        let connection = {
            let mut slot = self.connection.lock().await;
            if let Some(connection) = slot.as_ref().filter(|c| c.close_reason().is_none()) {
                connection.clone()
            } else {
                let dial = tokio::time::timeout(CALL_TIMEOUT, self.endpoint.connect(peer, RAFT_ALPN)).await;
                let connection = match dial {
                    Ok(Ok(connection)) => connection,
                    Ok(Err(e)) => return Err(Failure::Unreachable(CallError(e.to_string()))),
                    Err(_) => return Err(Failure::Unreachable(CallError("dialing timed out".into()))),
                };
                *slot = Some(connection.clone());
                connection
            }
        };
        let exchange = async {
            let (mut send, mut recv) = connection.open_bi().await?;
            write_message(&mut send, &call).await?;
            read_message::<Answer>(&mut recv).await
        };
        match tokio::time::timeout(CALL_TIMEOUT, exchange).await {
            Ok(Ok(answer)) => Ok(answer),
            Ok(Err(e)) => Err(Failure::Network(CallError(e.to_string()))),
            Err(_) => Err(Failure::Network(CallError("no answer in time".into()))),
        }
    }
}

enum Failure {
    Unreachable(CallError),
    Network(CallError),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable(e) | Self::Network(e) => e.fmt(f),
        }
    }
}

impl<E: std::error::Error> From<Failure> for RPCError<NodeId, BasicNode, E> {
    fn from(failure: Failure) -> Self {
        match failure {
            Failure::Unreachable(e) => RPCError::Unreachable(Unreachable::new(&e)),
            Failure::Network(e) => RPCError::Network(NetworkError::new(&e)),
        }
    }
}

fn unexpected<E: std::error::Error>() -> RPCError<NodeId, BasicNode, E> {
    RPCError::Network(NetworkError::new(&CallError("the member answered another call".into())))
}

impl RaftNetwork<TypeConfig> for Client {
    async fn append_entries(
        &mut self,
        request: AppendEntriesRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        match self.call(Call::Append(request)).await? {
            Answer::Append(result) => result.map_err(|e| RPCError::RemoteError(RemoteError::new(self.target, e))),
            _ => Err(unexpected()),
        }
    }

    async fn install_snapshot(
        &mut self,
        request: InstallSnapshotRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<InstallSnapshotResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId, InstallSnapshotError>>>
    {
        match self.call(Call::Snapshot(request)).await? {
            Answer::Snapshot(result) => result.map_err(|e| RPCError::RemoteError(RemoteError::new(self.target, e))),
            _ => Err(unexpected()),
        }
    }

    async fn vote(
        &mut self,
        request: VoteRequest<NodeId>,
        _option: RPCOption,
    ) -> Result<VoteResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        match self.call(Call::Vote(request)).await? {
            Answer::Vote(result) => result.map_err(|e| RPCError::RemoteError(RemoteError::new(self.target, e))),
            _ => Err(unexpected()),
        }
    }
}
