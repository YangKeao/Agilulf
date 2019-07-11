use super::{Database, Slice};
use agilulf_protocol::error::database_error::{DatabaseError, Result};

use std::collections::BTreeMap;
use std::sync::RwLock;
use futures::Future;
use std::pin::Pin;

#[derive(Default)]
pub struct MemDatabase {
    inner: RwLock<BTreeMap<Slice, Slice>>,
}

impl Database for MemDatabase {
    fn get(&self, key: Slice) -> Pin<Box<dyn Future<Output = Result<Slice>> + Send + '_>> {
        Box::pin(async move {
            match self.inner.read().unwrap().get(&key) {
                Some(value) => Ok(value.clone()), // TODO: clone here may be avoidable
                None => Err(DatabaseError::KeyNotFound),
            }
        })
    }

    fn put(&self, key: Slice, value: Slice) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            self.inner.write().unwrap().insert(key, value);
            Ok(())
        })
    }

    fn scan(&self, start: Slice, end: Slice) -> Pin<Box<dyn Future<Output = Result<Vec<Slice>>> + Send + '_>> {
        Box::pin(async move {
            Ok(self
                .inner
                .read()
                .unwrap()
                .range(start..end)
                .map(|(key, _)| key.clone()) // TODO: clone here may be avoidable
                .collect())
        })
    }

    fn delete(&self, key: Slice) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            match self.inner.write().unwrap().remove(&key) {
                Some(_) => Ok(()),
                None => Err(DatabaseError::KeyNotFound),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agilulf_protocol::Slice;

    #[bench]
    fn bench_put(b: &mut test::Bencher) {
        // TODO: need rewrite
        use rand::distributions::Standard;
        use rand::{thread_rng, Rng};;

        let db = MemDatabase::default();

        let keys: Vec<Vec<u8>> = (0..1000)
            .map(|_| thread_rng().sample_iter(&Standard).take(8).collect())
            .collect();

        let value: Vec<Vec<u8>> = (0..1000)
            .map(|_| thread_rng().sample_iter(&Standard).take(256).collect())
            .collect();

        b.iter(|| {
            for index in 0..1000 {
                db.put(Slice(keys[index].clone()), Slice(value[index].clone()))
                    .unwrap();
            }
        })
    }
}
