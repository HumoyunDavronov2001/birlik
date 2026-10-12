//! What a node remembers about its cluster (`protocol/CLUSTER.md`): the cluster it belongs to, its own membership
//! token and the members, in `state.redb` in the node's data directory; the cluster key, on admin devices, in
//! `cluster.key` next to it, readable only by the user.

use std::fmt;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use omnarc_network::EndpointId;
use omnarc_network::cluster::{ClusterId, ClusterKey, Right};
use omnarc_network::join::Joined;
use omnarc_protocol::v1;
use prost::Message as _;
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};

const META: TableDefinition<&str, &[u8]> = TableDefinition::new("meta");
const MEMBERS: TableDefinition<&[u8], &[u8]> = TableDefinition::new("members");

/// The rights of the device that creates a cluster.
pub const CREATOR_RIGHTS: [Right; 3] = [Right::Node, Right::User, Right::Admin];

/// An error of the node's state.
#[derive(Debug)]
pub struct Error(String);

impl Error {
    fn new(context: &str, error: impl fmt::Display) -> Self {
        Self(format!("{context}: {error}"))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// The cluster this node belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cluster {
    pub id: ClusterId,
    pub name: String,
    /// This node's membership token.
    pub token: Vec<u8>,
}

/// A member as this node knows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub id: EndpointId,
    pub device: v1::DeviceInfo,
    pub token: Vec<u8>,
    pub joined_unix_ms: i64,
    /// The last addresses the device was reached at.
    pub addrs: Vec<SocketAddr>,
}

/// The node's state on disk.
#[derive(Debug)]
pub struct Store {
    db: Database,
    dir: PathBuf,
}

impl Store {
    /// Opens the state in `dir` (the node's data directory), creating it on first use.
    ///
    /// # Errors
    /// The directory or the database cannot be created or opened.
    pub fn open(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir).map_err(|e| Error::new("creating the data directory", e))?;
        let db = Database::create(dir.join("state.redb")).map_err(|e| Error::new("opening state.redb", e))?;
        let tx = db.begin_write().map_err(|e| Error::new("state", e))?;
        tx.open_table(META).map_err(|e| Error::new("state", e))?;
        tx.open_table(MEMBERS).map_err(|e| Error::new("state", e))?;
        tx.commit().map_err(|e| Error::new("state", e))?;
        Ok(Self { db, dir: dir.to_path_buf() })
    }

    /// The cluster this node belongs to, if any.
    ///
    /// # Errors
    /// The state cannot be read.
    pub fn cluster(&self) -> Result<Option<Cluster>> {
        let tx = self.db.begin_read().map_err(|e| Error::new("state", e))?;
        let meta = tx.open_table(META).map_err(|e| Error::new("state", e))?;
        let get = |key: &str| -> Result<Option<Vec<u8>>> {
            Ok(meta.get(key).map_err(|e| Error::new("state", e))?.map(|value| value.value().to_vec()))
        };
        let (Some(id), Some(name), Some(token)) = (get("cluster_id")?, get("cluster_name")?, get("token")?) else {
            return Ok(None);
        };
        Ok(Some(Cluster {
            id: ClusterId::from_bytes(&id).map_err(|e| Error::new("state", e))?,
            name: String::from_utf8(name).map_err(|e| Error::new("state", e))?,
            token,
        }))
    }

    /// The cluster key, when this device is an admin that holds it.
    ///
    /// # Errors
    /// The key file exists but cannot be read.
    pub fn cluster_key(&self) -> Result<Option<ClusterKey>> {
        match std::fs::read(self.dir.join("cluster.key")) {
            Ok(bytes) => {
                let seed: [u8; 32] = bytes.try_into().map_err(|_| Error::new("cluster.key", "not a key"))?;
                Ok(Some(ClusterKey::from_bytes(&seed).map_err(|e| Error::new("cluster.key", e))?))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Error::new("reading cluster.key", e)),
        }
    }

    /// Creates a cluster named `name` with this device as its first member and admin.
    ///
    /// # Errors
    /// The node already belongs to a cluster, or the key or the state cannot be written.
    pub fn create_cluster(
        &self,
        name: &str,
        me: EndpointId,
        device: v1::DeviceInfo,
        now_unix_ms: i64,
    ) -> Result<Cluster> {
        if let Some(cluster) = self.cluster()? {
            return Err(Error::new("creating a cluster", format!("this device already belongs to {}", cluster.name)));
        }
        let key = ClusterKey::generate();
        let token =
            key.issue(me, &CREATOR_RIGHTS, now_unix_ms / 1000).map_err(|e| Error::new("creating a cluster", e))?;
        omnarc_network::write_private(&self.dir.join("cluster.key"), &key.to_bytes())
            .map_err(|e| Error::new("writing cluster.key", e))?;
        let cluster = Cluster { id: key.id(), name: name.to_owned(), token: token.clone() };
        self.save_cluster(&cluster)?;
        self.put_member(&Member { id: me, device, token, joined_unix_ms: now_unix_ms, addrs: vec![] })?;
        Ok(cluster)
    }

    /// Records the cluster this node has just joined, with the members it was told about.
    ///
    /// # Errors
    /// The node already belongs to a cluster, or the state cannot be written.
    pub fn save_joined(
        &self,
        joined: &Joined,
        me: EndpointId,
        device: v1::DeviceInfo,
        now_unix_ms: i64,
    ) -> Result<Cluster> {
        if let Some(cluster) = self.cluster()? {
            return Err(Error::new("joining", format!("this device already belongs to {}", cluster.name)));
        }
        let cluster = Cluster { id: joined.cluster, name: joined.name.clone(), token: joined.token.clone() };
        self.save_cluster(&cluster)?;
        for id in &joined.members {
            if *id == me {
                continue;
            }
            // tokens and devices of the others arrive through gossip
            self.put_member(&Member {
                id: *id,
                device: v1::DeviceInfo::default(),
                token: vec![],
                joined_unix_ms: 0,
                addrs: vec![],
            })?;
        }
        self.put_member(&Member {
            id: me,
            device,
            token: joined.token.clone(),
            joined_unix_ms: now_unix_ms,
            addrs: vec![],
        })?;
        Ok(cluster)
    }

    /// Adds or updates a member.
    ///
    /// # Errors
    /// The state cannot be written.
    pub fn put_member(&self, member: &Member) -> Result<()> {
        let record = v1::Member {
            id: Some(v1::NodeId { ed25519_public_key: member.id.as_bytes().to_vec() }),
            device: Some(member.device.clone()),
            membership_token: member.token.clone(),
            joined_unix_ms: member.joined_unix_ms,
            direct_addresses: member.addrs.iter().map(ToString::to_string).collect(),
        };
        let tx = self.db.begin_write().map_err(|e| Error::new("state", e))?;
        {
            let mut members = tx.open_table(MEMBERS).map_err(|e| Error::new("state", e))?;
            members
                .insert(member.id.as_bytes().as_slice(), record.encode_to_vec().as_slice())
                .map_err(|e| Error::new("state", e))?;
        }
        tx.commit().map_err(|e| Error::new("state", e))
    }

    /// Every member this node knows, in the order of their ids.
    ///
    /// # Errors
    /// The state cannot be read.
    pub fn members(&self) -> Result<Vec<Member>> {
        let tx = self.db.begin_read().map_err(|e| Error::new("state", e))?;
        let table = tx.open_table(MEMBERS).map_err(|e| Error::new("state", e))?;
        let mut members = Vec::new();
        for entry in table.iter().map_err(|e| Error::new("state", e))? {
            let (_, value) = entry.map_err(|e| Error::new("state", e))?;
            let record = v1::Member::decode(value.value()).map_err(|e| Error::new("state", e))?;
            let id = record.id.ok_or_else(|| Error::new("state", "a member without an id"))?;
            let bytes: [u8; 32] = id
                .ed25519_public_key
                .as_slice()
                .try_into()
                .map_err(|_| Error::new("state", "a malformed member id"))?;
            members.push(Member {
                id: EndpointId::from_bytes(&bytes).map_err(|e| Error::new("state", e))?,
                device: record.device.unwrap_or_default(),
                token: record.membership_token,
                joined_unix_ms: record.joined_unix_ms,
                addrs: record.direct_addresses.iter().filter_map(|addr| addr.parse().ok()).collect(),
            });
        }
        Ok(members)
    }

    /// The member with this id, if this node knows it.
    ///
    /// # Errors
    /// The state cannot be read.
    pub fn member(&self, id: EndpointId) -> Result<Option<Member>> {
        Ok(self.members()?.into_iter().find(|member| member.id == id))
    }

    /// Updates what this node knows about a member: its device and its addresses, keeping the rest; a member it did
    /// not know is added with an empty token.
    ///
    /// # Errors
    /// The state cannot be read or written.
    pub fn update_member(
        &self,
        id: EndpointId,
        device: Option<v1::DeviceInfo>,
        addrs: Option<Vec<SocketAddr>>,
    ) -> Result<()> {
        let mut member = self.member(id)?.unwrap_or(Member {
            id,
            device: v1::DeviceInfo::default(),
            token: vec![],
            joined_unix_ms: 0,
            addrs: vec![],
        });
        if let Some(device) = device {
            member.device = device;
        }
        if let Some(addrs) = addrs {
            member.addrs = addrs;
        }
        self.put_member(&member)
    }

    fn save_cluster(&self, cluster: &Cluster) -> Result<()> {
        let tx = self.db.begin_write().map_err(|e| Error::new("state", e))?;
        {
            let mut meta = tx.open_table(META).map_err(|e| Error::new("state", e))?;
            meta.insert("cluster_id", cluster.id.as_bytes().as_slice()).map_err(|e| Error::new("state", e))?;
            meta.insert("cluster_name", cluster.name.as_bytes()).map_err(|e| Error::new("state", e))?;
            meta.insert("token", cluster.token.as_slice()).map_err(|e| Error::new("state", e))?;
        }
        tx.commit().map_err(|e| Error::new("state", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omnarc_network::SecretKey;
    use omnarc_network::cluster;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("omnarc-state-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn device(name: &str) -> v1::DeviceInfo {
        v1::DeviceInfo { name: name.into(), ..Default::default() }
    }

    #[test]
    fn a_created_cluster_survives_reopening() {
        let dir = temp_dir("create");
        let me = SecretKey::generate().public();
        {
            let store = Store::open(&dir).unwrap();
            assert_eq!(store.cluster().unwrap(), None);
            let cluster = store.create_cluster("Home cluster", me, device("Desktop"), 1_000).unwrap();
            let membership = cluster::verify(cluster.id, me, &cluster.token, |_| false).unwrap();
            assert!(membership.has(Right::Admin));
            assert!(store.create_cluster("Another", me, device("Desktop"), 2_000).is_err(), "one cluster per device");
        }
        let store = Store::open(&dir).unwrap();
        let cluster = store.cluster().unwrap().unwrap();
        assert_eq!(cluster.name, "Home cluster");
        assert_eq!(store.cluster_key().unwrap().unwrap().id(), cluster.id);
        let members = store.members().unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].id, me);
        assert_eq!(members[0].device.name, "Desktop");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_joined_cluster_records_its_members_without_the_key() {
        let dir = temp_dir("join");
        let key = ClusterKey::generate();
        let inviter = SecretKey::generate().public();
        let me = SecretKey::generate().public();
        let joined = Joined {
            cluster: key.id(),
            name: "Home cluster".into(),
            token: key.issue(me, &[Right::Node, Right::User], 0).unwrap(),
            members: vec![inviter, me],
            inviter,
        };
        let store = Store::open(&dir).unwrap();
        store.save_joined(&joined, me, device("Laptop"), 5_000).unwrap();
        assert_eq!(store.cluster().unwrap().unwrap().id, key.id());
        assert!(store.cluster_key().unwrap().is_none(), "only the creator holds the key");
        let members = store.members().unwrap();
        assert_eq!(members.len(), 2);
        assert!(members.iter().any(|m| m.id == me && m.device.name == "Laptop" && m.joined_unix_ms == 5_000));
        assert!(members.iter().any(|m| m.id == inviter));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
