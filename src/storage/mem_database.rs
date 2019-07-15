use super::{Database, Slice};
use agilulf_protocol::error::database_error::{DatabaseError, Result};

use std::pin::Pin;

use futures::Future;
use agilulf_skiplist::skipmap::SkipMap;

#[derive(Clone)]
enum Value {
    NotExist,
    Slice(Slice)
}

impl Default for Value {
    fn default() -> Self {
        Value::NotExist
    }
}

#[derive(Default)]
pub struct MemDatabase {
    inner: SkipMap<Value>,
}

impl Database for MemDatabase {
    fn get(&self, key: Slice) -> Pin<Box<dyn Future<Output = Result<Slice>> + Send + '_>> {
        Box::pin(async move {
            match self.inner.find(&key) {
                Some(value) => {
                    match value {
                        Value::NotExist => Err(DatabaseError::KeyNotFound),
                        Value::Slice(value) => Ok(value)
                    }
                },
                None => Err(DatabaseError::KeyNotFound),
            }
        })
    }

    fn put(&self, key: Slice, value: Slice) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            self.inner.insert(&key, &Value::Slice(value));
            Ok(())
        })
    }

    fn scan(&self, start: Slice, end: Slice) -> Pin<Box<dyn Future<Output = Vec<(Slice, Slice)>> + Send + '_>> {
        Box::pin(async move {
            self
                .inner
                .scan(start..end)
                .into_iter()
                .filter_map(|(key, value)| {
                    match value {
                        Value::Slice(value) => Some((key, value)),
                        Value::NotExist => None,
                    }
                })
                .collect()
        })
    }

    fn delete(&self, key: Slice) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            self.inner.insert(&key, &Value::NotExist);
            Ok(())
        })
    }
}