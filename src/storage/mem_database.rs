use super::{Database, Slice};
use agilulf_protocol::error::database_error::{DatabaseError, Result};

use std::collections::BTreeMap;
use std::pin::Pin;

use futures::Future;
use futures::lock::Mutex;

#[derive(Default)]
pub struct MemDatabase {
    inner: Mutex<BTreeMap<Slice, Slice>>,
}

impl Database for MemDatabase {
    fn get(&self, key: Slice) -> Pin<Box<dyn Future<Output = Result<Slice>> + Send + '_>> {
        Box::pin(async move {
            match self.inner.lock().await.get(&key) {
                Some(value) => Ok(value.clone()), // TODO: clone here may be avoidable
                None => Err(DatabaseError::KeyNotFound),
            }
        })
    }

    fn put(&self, key: Slice, value: Slice) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            self.inner.lock().await.insert(key, value);
            Ok(())
        })
    }

    fn scan(&self, start: Slice, end: Slice) -> Pin<Box<dyn Future<Output = Result<Vec<Slice>>> + Send + '_>> {
        Box::pin(async move {
            Ok(self
                .inner
                .lock()
                .await
                .range(start..end)
                .map(|(key, _)| key.clone()) // TODO: clone here may be avoidable
                .collect())
        })
    }

    fn delete(&self, key: Slice) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            match self.inner.lock().await.remove(&key) {
                Some(_) => Ok(()),
                None => Err(DatabaseError::KeyNotFound),
            }
        })
    }
}