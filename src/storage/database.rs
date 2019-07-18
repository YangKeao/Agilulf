use super::log_manager::LogManager;
use super::manifest_manager::ManifestManager;
use super::mem_database::MemDatabase;
use super::{AsyncDatabase, SyncDatabase};
use agilulf_protocol::error::database_error::Result as DatabaseResult;
use agilulf_protocol::Slice;
use futures::Future;
use std::pin::Pin;
use std::path::Path;
use crate::storage::log_manager::LogError;

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
    fn restore(mut self, restore: bool) -> Self {
        self.restore = restore;
        self
    }
    fn base_dir(mut self, base_dir: String) -> Self {
        self.base_dir = base_dir;
        self
    }
    fn build(&self) -> BuildResult<Database> {
        let base_path = Path::new(&self.base_dir); // TODO: handle error here

        let log_path = base_path.join("log");
        let log_path = log_path.to_str().unwrap(); // TODO: handle error here

        let log_manager = match self.restore {
            true => {
                match LogManager::open(log_path) {
                    Ok(log_manager) => log_manager,
                    Err(err) => {
                        panic!() // TODO: handle error here
                    }
                }
            }
            false => {
                match LogManager::create_new(log_path) {
                    Ok(log_manager) => log_manager,
                    Err(err) => {
                        panic!() // TODO: handle error here
                    }
                }
            }
        };

        let mem_database = if self.restore {
            MemDatabase::restore_from_iterator(log_manager.iter())
        } else {
            MemDatabase::default()
        };

        Ok(Database {
            mem_database,
            log_manager
        })
    }
}

pub struct Database {
    mem_database: MemDatabase,
    log_manager: LogManager,
//    manifest_manager: ManifestManager,
}

impl AsyncDatabase for Database {
    fn get(&self, key: Slice) -> Pin<Box<dyn Future<Output = DatabaseResult<Slice>> + Send + '_>> {
        Box::pin(async move {
            SyncDatabase::get(&self.mem_database, key)
        })
    }

    fn put(&self, key: Slice, value: Slice) -> Pin<Box<dyn Future<Output = DatabaseResult<()>> + Send + '_>> {
        Box::pin(async move {
            SyncDatabase::put(&self.log_manager, key.clone(), value.clone());
            SyncDatabase::put(&self.mem_database, key, value)
        })
    }

    fn scan(&self, start: Slice, end: Slice) -> Pin<Box<dyn Future<Output = Vec<(Slice, Slice)>> + Send + '_>> {
        Box::pin(async move {
            SyncDatabase::scan(&self.mem_database, start, end)
        })
    }

    fn delete(&self, key: Slice) -> Pin<Box<dyn Future<Output = DatabaseResult<()>> + Send + '_>> {
        Box::pin(async move {
            SyncDatabase::delete(&self.mem_database, key.clone());
            SyncDatabase::delete(&self.mem_database, key)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agilulf_protocol::{Command, PutCommand};

    #[test]
    fn log_test() {
        let database = DatabaseBuilder::default()
            .restore(false)
            .build().unwrap();
        futures::executor::block_on(async move {
            database.put(Slice(b"HELLO".to_vec()), Slice(b"WORLD".to_vec())).await.unwrap();
        });

        let log_manager = LogManager::open("/var/tmp/agilulf/log").unwrap();
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
}