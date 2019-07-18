use super::sstable::PART_LENGTH;
use super::Result as DatabaseResult;
use super::SyncDatabase;
use crate::log::{JudgeReal, Result};
use crate::log::{LogIterator, LogManager};
use agilulf_protocol::error::database_error::DatabaseError;
use agilulf_protocol::{Command, DeleteCommand, PutCommand, Slice};
use memmap::{MmapMut, MmapOptions};
use std::mem::size_of;
use std::sync::atomic::{AtomicUsize, Ordering};

#[repr(packed)]
#[derive(Clone)]
struct RawRecord {
    pub real_flag: u8,
    pub key: [u8; 8],
    pub delete_flag: u8,
    pub value: [u8; 256],
}

impl JudgeReal for RawRecord {
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

pub struct DatabaseLogIter<'a> {
    log_iter: LogIterator<'a, RawRecord>,
}

impl<'a> Iterator for DatabaseLogIter<'a> {
    type Item = Command;

    fn next(&mut self) -> Option<Self::Item> {
        let next_entry = match self.log_iter.next() {
            Some(entry) => entry,
            None => return None,
        };

        Some(match next_entry.delete_flag {
            0 => Command::PUT(PutCommand {
                key: Slice(next_entry.key.to_vec()),
                value: Slice(next_entry.value.to_vec()),
            }),
            1 => Command::DELETE(DeleteCommand {
                key: Slice(next_entry.key.to_vec()),
            }),
            _ => unreachable!(),
        })
    }
}

pub struct DatabaseLog {
    log_manager: LogManager<RawRecord>,
}

impl DatabaseLog {
    pub fn create_new(path: &str) -> Result<DatabaseLog> {
        let log_manager = LogManager::create_new(path)?;
        Ok(DatabaseLog { log_manager })
    }
    pub fn open(path: &str) -> Result<DatabaseLog> {
        let log_manager = LogManager::open(path)?;
        Ok(DatabaseLog { log_manager })
    }

    pub fn iter(&self) -> DatabaseLogIter {
        DatabaseLogIter {
            log_iter: self.log_manager.iter(),
        }
    }
}

impl SyncDatabase for DatabaseLog {
    fn get(&self, key: Slice) -> DatabaseResult<Slice> {
        panic!("Cannot read from log directly")
    }

    fn put(&self, key: Slice, value: Slice) -> DatabaseResult<()> {
        let mut key_slice = [0u8; 8];
        key_slice[0..key.0.len()].clone_from_slice(key.0.as_slice());
        let mut value_slice = [0u8; 256];
        value_slice[0..value.0.len()].clone_from_slice(value.0.as_slice());

        let record = RawRecord {
            real_flag: 1,
            key: key_slice,
            delete_flag: 0,
            value: value_slice,
        };
        self.log_manager.add_entry(record);

        Ok(())
    }

    fn scan(&self, start: Slice, end: Slice) -> Vec<(Slice, Slice)> {
        panic!("Cannot read from log directly")
    }

    fn delete(&self, key: Slice) -> DatabaseResult<()> {
        let mut key_slice = [0u8; 8];
        key_slice[0..key.0.len()].clone_from_slice(key.0.as_slice());
        let mut value_slice = [0u8; 256];

        let record = RawRecord {
            real_flag: 1,
            key: key_slice,
            delete_flag: 1,
            value: value_slice,
        };
        self.log_manager.add_entry(record);

        Ok(())
    }
}
