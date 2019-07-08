use super::error::{DatabaseError, Result};
use super::{Database, Slice};

use std::collections::BTreeMap;
use std::sync::RwLock;

impl Database for RwLock<BTreeMap<Slice, Slice>> {
    fn get(&self, key: Slice) -> Result<Slice> {
        match self.read().unwrap().get(&key) {
            Some(value) => Ok(value.clone()), // TODO: clone here may be avoidable
            None => Err(DatabaseError::KeyNotFound),
        }
    }

    fn put(&self, key: Slice, value: Slice) -> Result<()> {
        self.write().unwrap().insert(key, value);
        Ok(())
    }

    fn scan(&self, start: Slice, end: Slice) -> Result<Vec<Slice>> {
        unimplemented!()
    }
}
