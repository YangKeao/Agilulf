pub mod mem_database;
mod sstable;

use agilulf_protocol::Slice;

use agilulf_protocol::error::database_error::Result;
use futures::Future;
use std::pin::Pin;

pub trait SyncDatabase: Send + Sync {
    fn get(&self, key: Slice) -> Result<Slice>;

    fn put(&self, key: Slice, value: Slice) -> Result<()>;

    fn scan(&self, start: Slice, end: Slice) -> Vec<(Slice, Slice)>;

    fn delete(&self, key: Slice) -> Result<()>;
}

pub trait AsyncDatabase: Send + Sync {
    fn get(&self, key: Slice) -> Pin<Box<dyn Future<Output = Result<Slice>> + Send + '_>>;

    fn put(
        &self,
        key: Slice,
        value: Slice,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    fn scan(
        &self,
        start: Slice,
        end: Slice,
    ) -> Pin<Box<dyn Future<Output = Vec<(Slice, Slice)>> + Send + '_>>;

    fn delete(&self, key: Slice) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
}

impl<T: SyncDatabase> AsyncDatabase for T {
    fn get(&self, key: Slice) -> Pin<Box<dyn Future<Output = Result<Slice>> + Send + '_>> {
        Box::pin(async move {
            self.get(key)
        })
    }

    fn put(
        &self,
        key: Slice,
        value: Slice,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            self.put(key, value)
        })
    }

    fn scan(
        &self,
        start: Slice,
        end: Slice,
    ) -> Pin<Box<dyn Future<Output = Vec<(Slice, Slice)>> + Send + '_>> {
        Box::pin(async move {
            self.scan(start, end)
        })
    }

    fn delete(&self, key: Slice) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            self.delete(key)
        })
    }
}