use super::database_log::DatabaseLog;
use super::manifest_manager::ManifestManager;
use super::mem_database::MemDatabase;
use super::{AsyncDatabase, SyncDatabase};
use agilulf_protocol::error::database_error::Result as DatabaseResult;
use agilulf_protocol::Slice;
use futures::Future;
use std::pin::Pin;
use std::path::Path;
use crossbeam::atomic::AtomicCell;
use crossbeam::sync::ShardedLock;
use std::collections::VecDeque;
use std::sync::Arc;

pub struct DatabaseBuilder {
    base_dir: String,
    restore: bool
}

impl Default for DatabaseBuilder {
    fn default() -> DatabaseBuilder {
        DatabaseBuilder {
            base_dir: "/var/tmp/agilulf".to_string(),
            restore: true,
        }
    }
}

quick_error! {
    #[derive(Debug)]
    pub enum BuildError {

    }
}
pub type BuildResult<T> = std::result::Result<T, BuildError>;

impl DatabaseBuilder {
    fn restore(&mut self, restore: bool) -> &mut Self {
        self.restore = restore;
        self
    }
    fn base_dir(&mut self, base_dir: String) -> &mut Self {
        self.base_dir = base_dir;
        self
    }
    fn build(&self) -> BuildResult<Database> {
        let base_path = Path::new(&self.base_dir); // TODO: handle error here

        let log_path = base_path.join("log");
        let log_path = log_path.to_str().unwrap(); // TODO: handle error here

        let log_length = 4 * 1024 * 2;

        let database_log = match self.restore {
            true => {
                match DatabaseLog::open(log_path, log_length) {
                    Ok(database_log) => database_log,
                    Err(err) => {
                        panic!() // TODO: handle error here
                    }
                }
            }
            false => {
                match DatabaseLog::create_new(log_path, log_length) {
                    Ok(database_log) => database_log,
                    Err(err) => {
                        panic!() // TODO: handle error here
                    }
                }
            }
        };

        let mem_database = if self.restore {
            MemDatabase::restore_from_iterator(database_log.iter())
        } else {
            MemDatabase::default()
        };

        Ok(Database {
            frozen_databases: ShardedLock::new(VecDeque::new()),
            mem_database: ShardedLock::new(Arc::new(mem_database)),
            database_log
        })
    }
}

pub struct Database {
    frozen_databases: ShardedLock<VecDeque<Arc<MemDatabase>>>,
    mem_database: ShardedLock<Arc<MemDatabase>>,
    database_log: DatabaseLog,
}

impl Database {
    fn check_mem_database(&self) {
        if self.mem_database.read().unwrap().large_enough() {
            let old_database = self.mem_database.read().unwrap().clone();
            let mut frozen_queue  = self.frozen_databases.write().unwrap();

            let new_database = MemDatabase::default();
            self.mem_database.write().unwrap().clone_from(&Arc::new(new_database));

            frozen_queue.push_front(old_database);
        }
    }
}

impl AsyncDatabase for Database {
    fn get(&self, key: Slice) -> Pin<Box<dyn Future<Output = DatabaseResult<Slice>> + Send + '_>> {
        Box::pin(async move {
            let mut ret = self.mem_database.read().unwrap().get_sync(key.clone());

            if ret.is_err() {
                for db in self.frozen_databases.read().unwrap().iter() {
                    let try_ret = db.get_sync(key.clone());
                    if try_ret.is_ok() {
                        return try_ret;
                    }
                }
            }

            ret
        })
    }

    fn put(&self, key: Slice, value: Slice) -> Pin<Box<dyn Future<Output = DatabaseResult<()>> + Send + '_>> {
        Box::pin(async move {
            self.database_log.put_sync(key.clone(), value.clone());
            let ret = self.mem_database.read().unwrap().put_sync(key, value);

            self.check_mem_database();

            ret
        })
    }

    fn scan(&self, start: Slice, end: Slice) -> Pin<Box<dyn Future<Output = Vec<(Slice, Slice)>> + Send + '_>> {
        Box::pin(async move {
            self.mem_database.read().unwrap().scan_sync(start, end)
        })
    }

    fn delete(&self, key: Slice) -> Pin<Box<dyn Future<Output = DatabaseResult<()>> + Send + '_>> {
        Box::pin(async move {
            self.database_log.delete_sync(key.clone());
            let ret = self.mem_database.read().unwrap().delete_sync(key);

            self.check_mem_database();

            ret
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agilulf_protocol::{Command, PutCommand};
    use rand::{thread_rng, Rng};
    use rand::distributions::Standard;

    fn generate_keys(num: usize) -> Vec<Vec<u8>> {
        (0..num).map(|_| {
            thread_rng().sample_iter(&Standard).take(8).collect()
        }).collect()
    }

    fn generate_values(num: usize) -> Vec<Vec<u8>> {
        (0..num).map(|_| {
            thread_rng().sample_iter(&Standard).take(256).collect()
        }).collect()
    }

    #[test]
    fn log_test() {
        let database = DatabaseBuilder::default()
            .restore(false)
            .build().unwrap();
        futures::executor::block_on(async move {
            database.put(Slice(b"HELLO".to_vec()), Slice(b"WORLD".to_vec())).await.unwrap();
        });

        let log_manager = DatabaseLog::open("/var/tmp/agilulf/log", 4 * 1024 * 2).unwrap();
        for command in log_manager.iter() {
            match command {
                Command::PUT(command) => {
                    assert_eq!(&command.key.0[0..5], b"HELLO");
                    assert_eq!(&command.value.0[0..5], b"WORLD");
                }
                _ => {
                    unreachable!()
                }
            }
        }

        let database = DatabaseBuilder::default()
            .restore(true)
            .build().unwrap();
        futures::executor::block_on(async move {
            let value = database.get(Slice(b"HELLO\0\0\0".to_vec())).await.unwrap();
            assert_eq!(&value.0[0..5], b"WORLD");
            database.put(Slice(b"HELLO2".to_vec()), Slice(b"WORLD".to_vec())).await.unwrap();
        });

        let database = DatabaseBuilder::default()
            .restore(true)
            .build().unwrap();
        futures::executor::block_on(async move {
            let value = database.get(Slice(b"HELLO2\0\0".to_vec())).await.unwrap();
            assert_eq!(&value.0[0..5], b"WORLD");
        });
    }

    #[test]
    fn frozen_test() {
        let keys = generate_keys(10 * 1024);
        let values = generate_values(10 * 1024);

        let database = DatabaseBuilder::default()
            .restore(false)
            .build().unwrap();

        futures::executor::block_on(async move {
            for index in 0..(5 * 1024) {
                database.put(Slice(keys[index].clone()), Slice(values[index].clone())).await.unwrap();
            }

            for index in 0..(5 * 1024) {
                let value = database.get(Slice(keys[index].clone())).await.unwrap();
                assert_eq!(value, Slice(values[index].clone()))
            }
        })
    }
}