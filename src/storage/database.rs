use super::database_log::DatabaseLog;
use super::error::{StorageError, StorageResult};
use super::manifest_manager::ManifestManager;
use super::mem_database::MemDatabase;
use super::merge::merge_iter;
use super::{AsyncDatabase, SyncDatabase};

use agilulf_protocol::error::database_error::{DatabaseError, Result as DatabaseResult};
use agilulf_protocol::Slice;

use crossbeam::sync::ShardedLock;
use futures::channel::mpsc::UnboundedSender;
use futures::Future;
use std::collections::VecDeque;
use std::path::Path;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub struct DatabaseBuilder {
    base_dir: String,
    restore: bool,
}

impl Default for DatabaseBuilder {
    fn default() -> DatabaseBuilder {
        DatabaseBuilder {
            base_dir: "/var/tmp/agilulf".to_string(),
            restore: true,
        }
    }
}

impl DatabaseBuilder {
    pub fn restore(&mut self, restore: bool) -> &mut Self {
        self.restore = restore;
        self
    }
    pub fn base_dir(&mut self, base_dir: String) -> &mut Self {
        self.base_dir = base_dir;
        self
    }
    pub fn build(&self) -> StorageResult<Database> {
        let base_path = Path::new(&self.base_dir);

        let log_path = base_path.join("log");
        let log_path = match log_path.to_str() {
            Some(str) => str,
            None => {
                log::error!("Path is not UTF-8");
                return Err(StorageError::UnicodeError);
            }
        };

        let log_length = 4 * 1024 * 2;

        let database_log = match self.restore {
            true => DatabaseLog::open(log_path, log_length)?,
            false => DatabaseLog::create_new(log_path, log_length)?,
        };

        let mem_database = if self.restore {
            MemDatabase::restore_from_iterator(database_log.iter())
        } else {
            MemDatabase::default()
        };

        let frozen_databases_queue = Arc::new(ShardedLock::new(VecDeque::new()));

        let manifest_manager = if self.restore {
            ManifestManager::open(self.base_dir.as_str(), frozen_databases_queue.clone())?
        } else {
            ManifestManager::create_new(self.base_dir.as_str(), frozen_databases_queue.clone())?
        };
        let freeze_notifier = manifest_manager.background_work();

        Ok(Database {
            frozen_databases: frozen_databases_queue,
            mem_database: ShardedLock::new(Arc::new(mem_database)),
            database_log: ShardedLock::new(Arc::new(database_log)),
            base_dir: self.base_dir.to_string(),
            log_counter: AtomicUsize::new(0),
            manifest_manager,
            freeze_notifier,
        })
    }
}

pub struct Database {
    frozen_databases: Arc<ShardedLock<VecDeque<Arc<MemDatabase>>>>,
    mem_database: ShardedLock<Arc<MemDatabase>>,
    database_log: ShardedLock<Arc<DatabaseLog>>,
    base_dir: String,
    log_counter: AtomicUsize,
    manifest_manager: ManifestManager,
    freeze_notifier: UnboundedSender<usize>,
}

impl Database {
    fn check_mem_database(&self) -> StorageResult<()> {
        if self.mem_database.read().unwrap().large_enough() {
            let base_path = Path::new(&self.base_dir);

            let old_database = self.mem_database.read().unwrap().clone();
            let mut frozen_queue = self.frozen_databases.write().unwrap();

            let new_database = MemDatabase::default();
            self.mem_database
                .write()
                .unwrap()
                .clone_from(&Arc::new(new_database));

            let log_id = self.log_counter.fetch_add(1, Ordering::SeqCst);
            let new_log_path = base_path.join(format!("log.{}", log_id));
            let new_log_path = match new_log_path.to_str() {
                Some(str) => str,
                None => {
                    log::error!("log path {:#?} is not UTF-8", new_log_path);
                    return Err(StorageError::UnicodeError);
                }
            };
            self.database_log.read().unwrap().rename(new_log_path);

            let log_path = base_path.join("log");
            let log_path = match log_path.to_str() {
                Some(str) => str,
                None => {
                    log::error!("log path {:#?} is not UTF-8", log_path);
                    return Err(StorageError::UnicodeError);
                }
            };
            let new_log = DatabaseLog::create_new(log_path, 4 * 1024 * 2)?;
            self.database_log
                .write()
                .unwrap()
                .clone_from(&Arc::new(new_log));

            frozen_queue.push_front(old_database);
            self.freeze_notifier.clone().unbounded_send(log_id);
        };

        Ok(())
    }
}

impl AsyncDatabase for Database {
    fn get(&self, key: Slice) -> Pin<Box<dyn Future<Output = DatabaseResult<Slice>> + Send + '_>> {
        Box::pin(async move {
            let ret = self.mem_database.read().unwrap().get_sync(key.clone());

            if ret.is_err() {
                for db in self.frozen_databases.read().unwrap().iter() {
                    let try_ret = db.get_sync(key.clone());
                    if try_ret.is_ok() {
                        return try_ret;
                    }
                }

                match self.manifest_manager.find_key(key.clone()) {
                    Some(value) => return Ok(value),
                    None => return Err(DatabaseError::KeyNotFound),
                };
            }

            ret
        })
    }

    fn put(
        &self,
        key: Slice,
        value: Slice,
    ) -> Pin<Box<dyn Future<Output = DatabaseResult<()>> + Send + '_>> {
        Box::pin(async move {
            self.database_log
                .read()
                .unwrap()
                .put_sync(key.clone(), value.clone());
            let ret = self.mem_database.read().unwrap().put_sync(key, value);

            self.check_mem_database();

            ret
        })
    }

    fn scan(
        &self,
        start: Slice,
        end: Slice,
    ) -> Pin<Box<dyn Future<Output = Vec<(Slice, Slice)>> + Send + '_>> {
        Box::pin(async move {
            let mut merge_vec = Vec::new();
            merge_vec.push(
                self.mem_database
                    .read()
                    .unwrap()
                    .scan_sync(start, end)
                    .into_iter(),
            );

            merge_iter(merge_vec).collect()
        })
    }

    fn delete(&self, key: Slice) -> Pin<Box<dyn Future<Output = DatabaseResult<()>> + Send + '_>> {
        Box::pin(async move {
            self.database_log.read().unwrap().delete_sync(key.clone());
            let ret = self.mem_database.read().unwrap().delete_sync(key);

            self.check_mem_database();

            ret
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agilulf_protocol::Command;
    use rand::distributions::Standard;
    use rand::{thread_rng, Rng};

    fn generate_keys(num: usize) -> Vec<Vec<u8>> {
        (0..num)
            .map(|_| thread_rng().sample_iter(&Standard).take(8).collect())
            .collect()
    }

    fn generate_values(num: usize) -> Vec<Vec<u8>> {
        (0..num)
            .map(|_| thread_rng().sample_iter(&Standard).take(256).collect())
            .collect()
    }

    #[test]
    fn log_test() {
        let database = DatabaseBuilder::default().restore(false).build().unwrap();
        futures::executor::block_on(async move {
            database
                .put(Slice(b"HELLO".to_vec()), Slice(b"WORLD".to_vec()))
                .await
                .unwrap();
        });

        let log_manager = DatabaseLog::open("/var/tmp/agilulf/log", 4 * 1024 * 2).unwrap();
        for command in log_manager.iter() {
            match command {
                Command::PUT(command) => {
                    assert_eq!(&command.key.0[0..5], b"HELLO");
                    assert_eq!(&command.value.0[0..5], b"WORLD");
                }
                _ => unreachable!(),
            }
        }

        let database = DatabaseBuilder::default().restore(true).build().unwrap();
        futures::executor::block_on(async move {
            let value = database.get(Slice(b"HELLO\0\0\0".to_vec())).await.unwrap();
            assert_eq!(&value.0[0..5], b"WORLD");
            database
                .put(Slice(b"HELLO2".to_vec()), Slice(b"WORLD".to_vec()))
                .await
                .unwrap();
        });

        let database = DatabaseBuilder::default().restore(true).build().unwrap();
        futures::executor::block_on(async move {
            let value = database.get(Slice(b"HELLO2\0\0".to_vec())).await.unwrap();
            assert_eq!(&value.0[0..5], b"WORLD");
        });
    }

    #[test]
    fn frozen_test() {
        let keys = generate_keys(10 * 1024);
        let values = generate_values(10 * 1024);

        let database = DatabaseBuilder::default().restore(false).build().unwrap();

        futures::executor::block_on(async move {
            for index in 0..(5 * 1024) {
                database
                    .put(Slice(keys[index].clone()), Slice(values[index].clone()))
                    .await
                    .unwrap();
            }

            for index in 0..(5 * 1024) {
                let value = database.get(Slice(keys[index].clone())).await.unwrap();
                assert_eq!(value, Slice(values[index].clone()))
            }
        })
    }

    #[test]
    fn scan_test() {
        let key = Slice(b"HELLO".to_vec());
        let key = &key;

        let database = DatabaseBuilder::default().restore(false).build().unwrap();

        futures::executor::block_on(async move {
            for index in 0..(5 * 1024) {
                let value = Slice(format!("WORLD{}", index).into_bytes());

                database.put(key.clone(), value).await.unwrap();
            }

            let ret = database
                .scan(Slice(b"HELL\0".to_vec()), Slice(b"HELLP".to_vec()))
                .await;
            assert_eq!(ret.len(), 1);
            let value = Slice(format!("WORLD{}", (5 * 1024 - 1)).into_bytes());
            assert_eq!(ret[0], (key.clone(), value));
        })
    }

    #[test]
    fn restore_after_big_request_test() {
        let keys = generate_keys(1024 * 16);
        let values = generate_values(1024 * 16);

        let keys = &keys;
        let values = &values;

        let database = DatabaseBuilder::default().restore(false).build().unwrap();
        futures::executor::block_on(async move {
            for index in 0..(1024 * 16) {
                let key = Slice(keys[index].clone());
                let value = Slice(values[index].clone());

                database.put(key, value).await.unwrap();
            }

            for index in 0..(1024 * 16) {
                let key = Slice(keys[index].clone());
                let value = Slice(values[index].clone());

                let ans = database.get(key).await.unwrap();
                assert_eq!(ans, value);
            }
        });

        let database = DatabaseBuilder::default().restore(true).build().unwrap();
        futures::executor::block_on(async move {
            for index in 0..(1024 * 16) {
                let key = Slice(keys[index].clone());
                let value = Slice(values[index].clone());

                let ans = database.get(key).await.unwrap();
                assert_eq!(ans, value);
            }
        });
    }
}
