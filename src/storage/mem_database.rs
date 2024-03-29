use super::{Slice, SyncDatabase};
use agilulf_protocol::{DatabaseError, DatabaseResult as Result};

use crate::storage::error::StorageResult;
use agilulf_protocol::Command;
use agilulf_skiplist::SkipMap;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering;

#[derive(Clone)]
enum Value {
    NotExist,
    Slice(Slice),
}

impl Default for Value {
    fn default() -> Self {
        Value::NotExist
    }
}

/// A simple RAM only database with skiplist as kernel.
///
/// The type of Value in skiplist is either NotExist (used for deleting element) or Slice.
pub struct MemDatabase {
    inner: AtomicPtr<SkipMap<Value>>,
}

impl MemDatabase {
    /// It can read from command iterator and run every command on MemDatabase. It is very useful for
    /// restoring data from log.
    pub fn restore_from_iterator<I: Iterator<Item = Command>>(
        iter: I,
    ) -> StorageResult<MemDatabase> {
        let mem_db = MemDatabase::default();
        for command in iter {
            match command {
                Command::PUT(command) => {
                    SyncDatabase::put_sync(&mem_db, command.key, command.value)?;
                }
                Command::DELETE(command) => {
                    SyncDatabase::delete_sync(&mem_db, command.key)?;
                }
                _ => unreachable!(),
            }
        }
        Ok(mem_db)
    }

    /// This function decide whether MemDatabase is too large. As every key and value is (256 + 8) bytes,
    /// nearly 1MB is the threshold.
    pub fn large_enough(&self) -> bool {
        unsafe { (*self.inner.load(Ordering::SeqCst)).len() > 4 * 1024 }
    }
}

impl Default for MemDatabase {
    fn default() -> Self {
        MemDatabase {
            inner: AtomicPtr::new(Box::into_raw(box SkipMap::default())),
        }
    }
}

impl Drop for MemDatabase {
    fn drop(&mut self) {
        let skip_map = self.inner.load(Ordering::SeqCst);
        unsafe { drop(Box::from_raw(skip_map)) }
    }
}

impl SyncDatabase for MemDatabase {
    fn get_sync(&self, key: Slice) -> Result<Slice> {
        unsafe {
            match (*self.inner.load(Ordering::SeqCst)).find(&key) {
                Some(value) => match value {
                    Value::NotExist => Err(DatabaseError::KeyNotFound),
                    Value::Slice(value) => Ok(value),
                },
                None => Err(DatabaseError::KeyNotFound),
            }
        }
    }

    fn put_sync(&self, key: Slice, value: Slice) -> Result<()> {
        unsafe {
            (*self.inner.load(Ordering::SeqCst)).insert(&key, &Value::Slice(value));
        }
        Ok(())
    }

    fn scan_sync(&self, start: Slice, end: Slice) -> Vec<(Slice, Slice)> {
        unsafe {
            (*self.inner.load(Ordering::SeqCst))
                .scan(start..end)
                .into_iter()
                .filter_map(|(key, value)| match value {
                    Value::Slice(value) => Some((key, value)),
                    Value::NotExist => None,
                })
                .collect()
        }
    }

    fn delete_sync(&self, key: Slice) -> Result<()> {
        unsafe {
            (*self.inner.load(Ordering::SeqCst)).insert(&key, &Value::NotExist);
        }
        Ok(())
    }
}
