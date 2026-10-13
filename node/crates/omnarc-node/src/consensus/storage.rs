//! The Raft log and the agreed state on disk: `raft.redb` in the node's data directory.
//!
//! Every write is committed before it is acknowledged, as Raft requires. The state machine is small — members,
//! revocations, a name and relays — so it is written whole after every applied batch, and a snapshot is that same
//! state with the log position it reflects.

// openraft's traits are async and its storage error is its own: a store that answers at once still implements them
#![allow(clippy::unused_async_trait_impl, clippy::result_large_err)]

use std::fmt::Debug;
use std::io::Cursor;
use std::ops::RangeBounds;
use std::path::Path;
use std::sync::{Arc, Mutex};

use openraft::storage::{LogFlushed, LogState, RaftLogReader, RaftLogStorage, RaftSnapshotBuilder, RaftStateMachine};
use openraft::{
    BasicNode, Entry, EntryPayload, LogId, OptionalSend, Snapshot, SnapshotMeta, StorageError, StorageIOError,
    StoredMembership, Vote,
};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use super::{Agreed, TypeConfig};
use crate::ring::BoxError;

const LOG: TableDefinition<u64, &[u8]> = TableDefinition::new("raft_log");
const META: TableDefinition<&str, &[u8]> = TableDefinition::new("raft_meta");

type NodeId = u64;
type Error = StorageError<NodeId>;

fn json<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(value)
}

/// Opens the log and the state machine in `path`, and a watch of the agreed state.
///
/// # Errors
/// The file cannot be opened or holds something else.
pub fn open(path: &Path) -> Result<(LogStore, StateMachine, watch::Receiver<Agreed>), BoxError> {
    open_db(Arc::new(Database::create(path)?))
}

/// [`open`] on a database already open — in memory, for the network simulation, where a restarted node finds
/// what it wrote before.
///
/// # Errors
/// The database holds something else.
pub fn open_db(db: Arc<Database>) -> Result<(LogStore, StateMachine, watch::Receiver<Agreed>), BoxError> {
    let tx = db.begin_write()?;
    tx.open_table(LOG)?;
    tx.open_table(META)?;
    tx.commit()?;
    let log = LogStore { db: Arc::clone(&db) };
    let state: Machine = read_meta(&db, "machine").map_err(|e| e.to_string())?.unwrap_or_default();
    let (sender, receiver) = watch::channel(state.agreed.clone());
    let machine = StateMachine { db, state: Arc::new(Mutex::new(state)), sender: Arc::new(sender) };
    Ok((log, machine, receiver))
}

fn read_meta<T: for<'de> Deserialize<'de>>(db: &Database, key: &str) -> Result<Option<T>, Error> {
    let tx = db.begin_read().map_err(|e| StorageIOError::read(&e))?;
    let table = tx.open_table(META).map_err(|e| StorageIOError::read(&e))?;
    let Some(value) = table.get(key).map_err(|e| StorageIOError::read(&e))? else {
        return Ok(None);
    };
    Ok(Some(serde_json::from_slice(value.value()).map_err(|e| StorageIOError::read(&e))?))
}

fn write_meta<T: Serialize>(db: &Database, key: &str, value: &T) -> Result<(), Error> {
    let bytes = json(value).map_err(|e| StorageIOError::write(&e))?;
    let tx = db.begin_write().map_err(|e| StorageIOError::write(&e))?;
    {
        let mut table = tx.open_table(META).map_err(|e| StorageIOError::write(&e))?;
        table.insert(key, bytes.as_slice()).map_err(|e| StorageIOError::write(&e))?;
    }
    tx.commit().map_err(|e| StorageIOError::write(&e))?;
    Ok(())
}

/// The Raft log.
#[derive(Clone, Debug)]
pub struct LogStore {
    db: Arc<Database>,
}

impl RaftLogReader<TypeConfig> for LogStore {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> Result<Vec<Entry<TypeConfig>>, Error> {
        let tx = self.db.begin_read().map_err(|e| StorageIOError::read_logs(&e))?;
        let table = tx.open_table(LOG).map_err(|e| StorageIOError::read_logs(&e))?;
        let mut entries = Vec::new();
        for item in table.range(range).map_err(|e| StorageIOError::read_logs(&e))? {
            let (_, value) = item.map_err(|e| StorageIOError::read_logs(&e))?;
            entries.push(serde_json::from_slice(value.value()).map_err(|e| StorageIOError::read_logs(&e))?);
        }
        Ok(entries)
    }
}

impl RaftLogStorage<TypeConfig> for LogStore {
    type LogReader = Self;

    async fn get_log_state(&mut self) -> Result<LogState<TypeConfig>, Error> {
        let last_purged_log_id: Option<LogId<NodeId>> = read_meta(&self.db, "purged")?;
        let tx = self.db.begin_read().map_err(|e| StorageIOError::read_logs(&e))?;
        let table = tx.open_table(LOG).map_err(|e| StorageIOError::read_logs(&e))?;
        let last = table.last().map_err(|e| StorageIOError::read_logs(&e))?;
        let last_log_id = match last {
            Some((_, value)) => {
                let entry: Entry<TypeConfig> =
                    serde_json::from_slice(value.value()).map_err(|e| StorageIOError::read_logs(&e))?;
                Some(entry.log_id)
            }
            None => last_purged_log_id,
        };
        Ok(LogState { last_purged_log_id, last_log_id })
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        self.clone()
    }

    async fn save_vote(&mut self, vote: &Vote<NodeId>) -> Result<(), Error> {
        write_meta(&self.db, "vote", vote)
    }

    async fn read_vote(&mut self) -> Result<Option<Vote<NodeId>>, Error> {
        read_meta(&self.db, "vote")
    }

    async fn save_committed(&mut self, committed: Option<LogId<NodeId>>) -> Result<(), Error> {
        write_meta(&self.db, "committed", &committed)
    }

    async fn read_committed(&mut self) -> Result<Option<LogId<NodeId>>, Error> {
        Ok(read_meta::<Option<LogId<NodeId>>>(&self.db, "committed")?.flatten())
    }

    async fn append<I>(&mut self, entries: I, callback: LogFlushed<TypeConfig>) -> Result<(), Error>
    where
        I: IntoIterator<Item = Entry<TypeConfig>> + OptionalSend,
        I::IntoIter: OptionalSend,
    {
        let tx = self.db.begin_write().map_err(|e| StorageIOError::write_logs(&e))?;
        {
            let mut table = tx.open_table(LOG).map_err(|e| StorageIOError::write_logs(&e))?;
            for entry in entries {
                let bytes = json(&entry).map_err(|e| StorageIOError::write_logs(&e))?;
                table.insert(entry.log_id.index, bytes.as_slice()).map_err(|e| StorageIOError::write_logs(&e))?;
            }
        }
        // durable before Raft hears it is written
        tx.commit().map_err(|e| StorageIOError::write_logs(&e))?;
        callback.log_io_completed(Ok(()));
        Ok(())
    }

    async fn truncate(&mut self, log_id: LogId<NodeId>) -> Result<(), Error> {
        let tx = self.db.begin_write().map_err(|e| StorageIOError::write_logs(&e))?;
        {
            let mut table = tx.open_table(LOG).map_err(|e| StorageIOError::write_logs(&e))?;
            table.retain_in(log_id.index.., |_, _| false).map_err(|e| StorageIOError::write_logs(&e))?;
        }
        tx.commit().map_err(|e| StorageIOError::write_logs(&e))?;
        Ok(())
    }

    async fn purge(&mut self, log_id: LogId<NodeId>) -> Result<(), Error> {
        write_meta(&self.db, "purged", &Some(log_id))?;
        let tx = self.db.begin_write().map_err(|e| StorageIOError::write_logs(&e))?;
        {
            let mut table = tx.open_table(LOG).map_err(|e| StorageIOError::write_logs(&e))?;
            table.retain_in(..=log_id.index, |_, _| false).map_err(|e| StorageIOError::write_logs(&e))?;
        }
        tx.commit().map_err(|e| StorageIOError::write_logs(&e))?;
        Ok(())
    }
}

/// The state machine as it is kept on disk.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Machine {
    last_applied: Option<LogId<NodeId>>,
    membership: StoredMembership<NodeId, BasicNode>,
    agreed: Agreed,
    snapshots: u64,
}

/// A snapshot as it is kept on disk.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredSnapshot {
    meta: SnapshotMeta<NodeId, BasicNode>,
    data: Vec<u8>,
}

/// The agreed state, applied from the log.
#[derive(Clone, Debug)]
pub struct StateMachine {
    db: Arc<Database>,
    state: Arc<Mutex<Machine>>,
    sender: Arc<watch::Sender<Agreed>>,
}

impl StateMachine {
    fn machine(&self) -> Machine {
        self.state.lock().expect("state machine").clone()
    }

    fn save(&self, machine: Machine) -> Result<(), Error> {
        write_meta(&self.db, "machine", &machine)?;
        let agreed = machine.agreed.clone();
        *self.state.lock().expect("state machine") = machine;
        self.sender.send_replace(agreed);
        Ok(())
    }
}

impl RaftSnapshotBuilder<TypeConfig> for StateMachine {
    async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfig>, Error> {
        let mut machine = self.machine();
        machine.snapshots += 1;
        let data = json(&machine.agreed).map_err(|e| StorageIOError::write_snapshot(None, &e))?;
        let index = machine.last_applied.map_or(0, |id| id.index);
        let meta = SnapshotMeta {
            last_log_id: machine.last_applied,
            last_membership: machine.membership.clone(),
            snapshot_id: format!("{index}-{}", machine.snapshots),
        };
        write_meta(&self.db, "snapshot", &StoredSnapshot { meta: meta.clone(), data: data.clone() })?;
        self.save(machine)?;
        Ok(Snapshot { meta, snapshot: Box::new(Cursor::new(data)) })
    }
}

impl RaftStateMachine<TypeConfig> for StateMachine {
    type SnapshotBuilder = Self;

    async fn applied_state(&mut self) -> Result<(Option<LogId<NodeId>>, StoredMembership<NodeId, BasicNode>), Error> {
        let machine = self.machine();
        Ok((machine.last_applied, machine.membership))
    }

    async fn apply<I>(&mut self, entries: I) -> Result<Vec<()>, Error>
    where
        I: IntoIterator<Item = Entry<TypeConfig>> + OptionalSend,
        I::IntoIter: OptionalSend,
    {
        let mut machine = self.machine();
        let mut responses = Vec::new();
        for entry in entries {
            machine.last_applied = Some(entry.log_id);
            match entry.payload {
                EntryPayload::Blank => {}
                EntryPayload::Normal(command) => machine.agreed.apply(&command),
                EntryPayload::Membership(membership) => {
                    machine.membership = StoredMembership::new(Some(entry.log_id), membership);
                }
            }
            responses.push(());
        }
        self.save(machine)?;
        Ok(responses)
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        self.clone()
    }

    async fn begin_receiving_snapshot(&mut self) -> Result<Box<Cursor<Vec<u8>>>, Error> {
        Ok(Box::new(Cursor::new(Vec::new())))
    }

    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<NodeId, BasicNode>,
        snapshot: Box<Cursor<Vec<u8>>>,
    ) -> Result<(), Error> {
        let data = snapshot.into_inner();
        let agreed: Agreed =
            serde_json::from_slice(&data).map_err(|e| StorageIOError::read_snapshot(Some(meta.signature()), &e))?;
        write_meta(&self.db, "snapshot", &StoredSnapshot { meta: meta.clone(), data })?;
        let mut machine = self.machine();
        machine.last_applied = meta.last_log_id;
        machine.membership = meta.last_membership.clone();
        machine.agreed = agreed;
        self.save(machine)
    }

    async fn get_current_snapshot(&mut self) -> Result<Option<Snapshot<TypeConfig>>, Error> {
        let stored: Option<StoredSnapshot> = read_meta(&self.db, "snapshot")?;
        Ok(stored.map(|s| Snapshot { meta: s.meta, snapshot: Box::new(Cursor::new(s.data)) }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // openraft's own suite of the storage contract: logs, votes, purging, applying, snapshots
    #[test]
    fn the_storage_keeps_the_raft_contract() {
        struct Builder;
        impl openraft::testing::StoreBuilder<TypeConfig, LogStore, StateMachine, tempdir::Dir> for Builder {
            async fn build(&self) -> Result<(tempdir::Dir, LogStore, StateMachine), StorageError<NodeId>> {
                let dir = tempdir::Dir::new();
                let (log, machine, _) = open(&dir.0.join("raft.redb"))
                    .map_err(|e| StorageIOError::write(&std::io::Error::other(e.to_string())))?;
                Ok((dir, log, machine))
            }
        }
        openraft::testing::Suite::test_all(Builder).unwrap();
    }

    mod tempdir {
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicU32, Ordering};

        static NEXT: AtomicU32 = AtomicU32::new(0);

        /// A directory removed when dropped.
        #[derive(Debug)]
        pub struct Dir(pub PathBuf);

        impl Dir {
            pub fn new() -> Self {
                let n = NEXT.fetch_add(1, Ordering::Relaxed);
                let dir = std::env::temp_dir().join(format!("omnarc-raft-{}-{n}", std::process::id()));
                let _ = std::fs::remove_dir_all(&dir);
                std::fs::create_dir_all(&dir).unwrap();
                Self(dir)
            }
        }

        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }
}
