use super::{Database, Slice};
use agilulf_protocol::error::database_error::{DatabaseError, Result};

use std::collections::BTreeMap;
use std::sync::RwLock;

#[derive(Default)]
pub struct MemDatabase {
    inner: RwLock<BTreeMap<Slice, Slice>>,
}

impl Database for MemDatabase {
    fn get(&self, key: Slice) -> Result<Slice> {
        match self.inner.read().unwrap().get(&key) {
            Some(value) => Ok(value.clone()), // TODO: clone here may be avoidable
            None => Err(DatabaseError::KeyNotFound),
        }
    }

    fn put(&self, key: Slice, value: Slice) -> Result<()> {
        self.inner.write().unwrap().insert(key, value);
        Ok(())
    }

    fn scan(&self, start: Slice, end: Slice) -> Result<Vec<Slice>> {
        Ok(self
            .inner
            .read()
            .unwrap()
            .range(start..end)
            .map(|(key, _)| key.clone()) // TODO: clone here may be avoidable
            .collect())
    }

    fn delete(&self, key: Slice) -> Result<()> {
        match self.inner.write().unwrap().remove(&key) {
            Some(_) => Ok(()),
            None => Err(DatabaseError::KeyNotFound),
        }
    }
}
