use super::database_log::DatabaseLog;
use super::sstable::SSTable;
use crate::log::{JudgeReal, LogManager};
use crate::{MemDatabase, AsyncDatabase};
use crossbeam::sync::ShardedLock;
use futures::task::{Spawn, SpawnExt, LocalSpawn, LocalSpawnExt};
use memmap::{MmapMut, MmapOptions};
use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};
use std::sync::atomic::Ordering;
use std::path::Path;
use crossbeam::channel::{unbounded, Sender};
use futures::stream::StreamExt;
use futures::executor::LocalPool;
use agilulf_protocol::Slice;
use std::pin::Pin;
use futures::Future;
use crate::storage::SyncDatabase;

#[repr(packed)]
#[derive(Clone)]
pub struct RawManifestLogEntry {
    real_flag: u8,
    add_flag: u8,
    level: u8,
    id: u8,
}

impl JudgeReal for RawManifestLogEntry {
    fn is_real(&self) -> bool {
        self.real_flag == 1
    }

    fn set_real(&mut self, real: bool) {
        if real {
            self.real_flag = 1;
        } else {
            self.real_flag = 0;
        }
    }
}

pub struct ManifestManager {
    base_dir: String,
    log_manager: LogManager<RawManifestLogEntry>,
    frozen_databases: Arc<ShardedLock<VecDeque<Arc<MemDatabase>>>>,
    sstables: Arc<[ShardedLock<BTreeMap<usize, SSTable>>; 6]>, // TODO: a concurrent RwLock may be better
    level_counter: Arc<[AtomicUsize; 6]>
}

impl ManifestManager {
    pub fn create_new(
        base_dir: &str,
        frozen_databases: Arc<ShardedLock<VecDeque<Arc<MemDatabase>>>>,
    ) -> ManifestManager {
        let base_path = Path::new(base_dir); // TODO: handle error here
        let base_path = base_path.join("MANIFEST");

        let manifest_path = base_path.to_str().unwrap();

        ManifestManager {
            base_dir: base_dir.to_string(),
            log_manager: LogManager::create_new(manifest_path, 4 * 1024).unwrap(), // TODO: handle error here
            frozen_databases,
            sstables: Arc::new([ShardedLock::new(BTreeMap::new()),
                ShardedLock::new(BTreeMap::new()),
                ShardedLock::new(BTreeMap::new()),
                ShardedLock::new(BTreeMap::new()),
                ShardedLock::new(BTreeMap::new()),
                ShardedLock::new(BTreeMap::new())]),
            level_counter: Arc::new([AtomicUsize::new(0),
                AtomicUsize::new(0),
                AtomicUsize::new(0),
                AtomicUsize::new(0),
                AtomicUsize::new(0),
                AtomicUsize::new(0)]) // TODO: use macro to avoid these redundant codes
        }
    }

    pub fn open(
        base_dir: &str,
        frozen_databases: Arc<ShardedLock<VecDeque<Arc<MemDatabase>>>>,
    ) -> ManifestManager {
        let base_path = Path::new(base_dir); // TODO: handle error here
        let base_path = base_path.join("MANIFEST");

        let manifest_path = base_path.to_str().unwrap();

        ManifestManager {
            base_dir: base_dir.to_string(),
            log_manager: LogManager::open(manifest_path, 4 * 1024).unwrap(), // TODO: handle error here
            frozen_databases,
            sstables: Arc::new([ShardedLock::new(BTreeMap::new()),
                ShardedLock::new(BTreeMap::new()),
                ShardedLock::new(BTreeMap::new()),
                ShardedLock::new(BTreeMap::new()),
                ShardedLock::new(BTreeMap::new()),
                ShardedLock::new(BTreeMap::new())]),
            level_counter: Arc::new([AtomicUsize::new(0),
                AtomicUsize::new(0),
                AtomicUsize::new(0),
                AtomicUsize::new(0),
                AtomicUsize::new(0),
                AtomicUsize::new(0)]) // TODO: restore sstables and level_counter from log
        }
    }

    fn compact<S: Spawn>(&self, mut spawner: S) {}

    pub fn freeze(&self) -> Sender<()> {
        let frozen_databases = self.frozen_databases.clone();

        let (sender, mut receiver) = unbounded::<()>();

        let base_dir = self.base_dir.clone();
        let level_counter = self.level_counter.clone();
        let sstables = self.sstables.clone();

        std::thread::spawn(move || {
            futures::executor::block_on(async move {
                let base_path = Path::new(&base_dir);
                loop {
                    receiver.recv();
                    let db_guard = frozen_databases.read().unwrap();
                    match db_guard.back() {
                        Some(db) => {
                            let sstable = SSTable::from(db.clone());
                            drop(db_guard);

                            let id = level_counter[0].fetch_add(1, Ordering::SeqCst);

                            let table_path = base_path.join(format!{"0_{}", id});
                            sstable.save(table_path.to_str().unwrap()).await; // TODO: handle error here

                            sstables[0].write().unwrap().insert(id, sstable);
                            frozen_databases.write().unwrap().pop_back();
                        }
                        None => {

                        }
                    }
                }
            })
        });

        sender
    }

    pub fn find_key(&self, key: Slice) -> Option<Slice> {
        for level in 0..6 {
            let level = self.sstables[level].read().unwrap();
            for (id, table) in level.iter() {
                match table.get_sync(key.clone()) {
                    Ok(value) => return Some(value),
                    Err(_) => {}
                }
            }
        }
        None
    }
}