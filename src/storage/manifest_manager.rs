use super::error::{StorageError, StorageResult};
use super::sstable::SSTable;
use crate::log::{JudgeReal, LogManager};
use crate::storage::SyncDatabase;
use crate::MemDatabase;
use super::merge::merge_iter;

use agilulf_protocol::Slice;
use crossbeam::sync::ShardedLock;
use futures::channel::mpsc::{unbounded, UnboundedSender};
use futures::executor::LocalPool;
use futures::stream::StreamExt;
use futures::task::LocalSpawnExt;

use std::collections::{BTreeMap, VecDeque};
use std::path::Path;

use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;

#[repr(packed)]
#[derive(Clone)]
pub struct RawManifestLogEntry {
    pub real_flag: u8,
    pub add_flag: u8,
    pub level: u8,
    pub id: u8,
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
    log_manager: Arc<LogManager<RawManifestLogEntry>>,
    frozen_databases: Arc<ShardedLock<VecDeque<Arc<MemDatabase>>>>,
    sstables: Arc<[ShardedLock<BTreeMap<usize, SSTable>>; 6]>, // TODO: a concurrent RwLock may be better
    level_counter: Arc<[AtomicUsize; 6]>,
}

impl ManifestManager {
    pub fn create_new(
        base_dir: &str,
        frozen_databases: Arc<ShardedLock<VecDeque<Arc<MemDatabase>>>>,
    ) -> StorageResult<ManifestManager> {
        let base_path = Path::new(base_dir);
        let manifest_path = base_path.join("MANIFEST");

        let manifest_path = match manifest_path.to_str() {
            Some(str) => str,
            None => {
                log::error!("Manifest path is not UTF-8: {:#?}", manifest_path);
                return Err(StorageError::UnicodeError);
            }
        };

        Ok(ManifestManager {
            base_dir: base_dir.to_string(),
            log_manager: Arc::new(LogManager::create_new(manifest_path, 4 * 1024)?),
            frozen_databases,
            sstables: Arc::new([
                ShardedLock::new(BTreeMap::new()),
                ShardedLock::new(BTreeMap::new()),
                ShardedLock::new(BTreeMap::new()),
                ShardedLock::new(BTreeMap::new()),
                ShardedLock::new(BTreeMap::new()),
                ShardedLock::new(BTreeMap::new()),
            ]),
            level_counter: Arc::new([
                AtomicUsize::new(0),
                AtomicUsize::new(0),
                AtomicUsize::new(0),
                AtomicUsize::new(0),
                AtomicUsize::new(0),
                AtomicUsize::new(0),
            ]), // TODO: use macro to avoid these redundant codes
        })
    }

    pub fn open(
        base_dir: &str,
        frozen_databases: Arc<ShardedLock<VecDeque<Arc<MemDatabase>>>>,
    ) -> StorageResult<ManifestManager> {
        let base_path = Path::new(base_dir);
        let manifest_path = base_path.join("MANIFEST");

        let manifest_path = match manifest_path.to_str() {
            Some(str) => str,
            None => {
                log::error!("Manifest path is not UTF-8: {:#?}", manifest_path);
                return Err(StorageError::UnicodeError);
            }
        };
        let log_manager: Arc<LogManager<RawManifestLogEntry>> =
            Arc::new(LogManager::open(manifest_path, 4 * 1024)?);

        let sstables = Arc::new([
            ShardedLock::new(BTreeMap::new()),
            ShardedLock::new(BTreeMap::new()),
            ShardedLock::new(BTreeMap::new()),
            ShardedLock::new(BTreeMap::new()),
            ShardedLock::new(BTreeMap::new()),
            ShardedLock::new(BTreeMap::new()),
        ]);

        let level_counter = Arc::new([
            AtomicUsize::new(0),
            AtomicUsize::new(0),
            AtomicUsize::new(0),
            AtomicUsize::new(0),
            AtomicUsize::new(0),
            AtomicUsize::new(0),
        ]);

        for log in log_manager.iter() {
            match log.add_flag {
                1 => {
                    if log.level >= 6 {
                        return Err(StorageError::ManifestLogFormatError);
                    }

                    let table_path = base_path.join(format!("sstable_{}_{}", log.level, log.id));
                    let sstable_file = std::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open(table_path)?;
                    let sstable = SSTable::open(sstable_file)?;
                    sstables
                        .get(log.level as usize)
                        .unwrap() // unwrap here is totally safe
                        .write()
                        .unwrap()
                        .insert(log.id as usize, sstable);
                    level_counter
                        .get(log.level as usize)
                        .unwrap() // unwrap here is totally safe
                        .fetch_max(log.id as usize, Ordering::SeqCst);
                }
                0 => {
                    if log.level >= 6 {
                        return Err(StorageError::ManifestLogFormatError);
                    }

                    sstables
                        .get(log.level as usize)
                        .unwrap() // unwrap here is totally safe
                        .write()
                        .unwrap()
                        .remove(&(log.id as usize));
                }
                _ => unreachable!(),
            }
        }

        Ok(ManifestManager {
            base_dir: base_dir.to_string(),
            log_manager,
            frozen_databases,
            sstables,
            level_counter,
        })
    }

    //    fn compact<S: Spawn>(&self, _spawner: S) {
    //        unimplemented!()
    //    }

    pub fn background_work(&self) -> StorageResult<UnboundedSender<usize>> {
        let frozen_databases = self.frozen_databases.clone();

        let (freeze_sender, freeze_receiver) = unbounded::<usize>();
        let mut freeze_receiver = freeze_receiver.fuse();

        let base_dir = self.base_dir.clone();
        let level_counter = self.level_counter.clone();
        let sstables = self.sstables.clone();
        let log_manager = self.log_manager.clone();

        std::thread::Builder::new()
            .name("background_worker".to_string())
            .spawn(move || {
                let mut local_pool = LocalPool::new();
                let spawn_result = local_pool.spawner().spawn_local(async move {
                    let base_path = Path::new(&base_dir);
                    loop {
                        let newest_log_id = match freeze_receiver.next().await {
                            Some(id) => id,
                            None => {
                                break;
                            }
                        };
                        let new_log_path = base_path.join(format!("log.{}", newest_log_id));

                        let db_guard = frozen_databases.read().unwrap();
                        match db_guard.back() {
                            Some(db) => {
                                let sstable = SSTable::from(db.clone());
                                drop(db_guard);

                                let id = level_counter[0].fetch_add(1, Ordering::SeqCst);

                                let table_path = base_path.join(format! {"sstable_0_{}", id});
                                let table_path = match table_path.to_str() {
                                    Some(str) => str,
                                    None => {
                                        log::error!("Table path is not UTF-8: {:#?}", table_path);
                                        continue;
                                    }
                                };
                                match sstable.save(table_path).await {
                                    Ok(()) => {}
                                    Err(err) => {
                                        log::error!("Error while storing SSTable: {}", err);
                                        continue;
                                    }
                                }

                                log_manager.add_entry(RawManifestLogEntry {
                                    real_flag: 1,
                                    add_flag: 1,
                                    level: 0,
                                    id: id as u8,
                                });

                                sstables[0].write().unwrap().insert(id, sstable);
                                frozen_databases.write().unwrap().pop_back();

                                match std::fs::remove_file(new_log_path) {
                                    Ok(()) => {}
                                    Err(err) => {
                                        log::error!("Error while remove log: {}", err);
                                        continue;
                                    }
                                };
                            }
                            None => {}
                        }
                    }
                });

                match spawn_result {
                    Ok(()) => {}
                    Err(err) => log::error!("Error while spawning: {}", err),
                }

                local_pool.run();
            })?;

        Ok(freeze_sender)
    }

    pub fn find_key(&self, key: Slice) -> Option<Slice> {
        for level in 0..6 {
            let level = self.sstables[level].read().unwrap();
            for (_id, table) in level.iter() {
                match table.get_sync(key.clone()) {
                    Ok(value) => return Some(value),
                    Err(_) => {}
                }
            }
        }
        None
    }

    pub fn scan(&self, start: Slice, end: Slice) -> impl Iterator<Item=(Slice, Slice)> {
        let mut merge_vec = Vec::new();
        for level in 0..6 {
            let level = self.sstables[level].read().unwrap();
            for (_id, table) in level.iter() {
                merge_vec.push(table.scan_sync(start.clone(), end.clone()).into_iter())
            }
        }
        merge_iter(merge_vec)
    }
}
