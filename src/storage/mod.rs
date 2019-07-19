pub mod database;
mod database_log;
pub mod error;
mod manifest_manager;
pub mod mem_database;
mod merge;
mod sstable;

use agilulf_protocol::Slice;

use agilulf_protocol::DatabaseResult as Result;
use futures::Future;
use std::pin::Pin;

pub use database::{Database, DatabaseBuilder};

pub trait SyncDatabase: Send + Sync {
    fn get_sync(&self, key: Slice) -> Result<Slice>;

    fn put_sync(&self, key: Slice, value: Slice) -> Result<()>;

    fn scan_sync(&self, start: Slice, end: Slice) -> Vec<(Slice, Slice)>;

    fn delete_sync(&self, key: Slice) -> Result<()>;
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
        Box::pin(async move { self.get_sync(key) })
    }

    fn put(
        &self,
        key: Slice,
        value: Slice,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move { self.put_sync(key, value) })
    }

    fn scan(
        &self,
        start: Slice,
        end: Slice,
    ) -> Pin<Box<dyn Future<Output = Vec<(Slice, Slice)>> + Send + '_>> {
        Box::pin(async move { self.scan_sync(start, end) })
    }

    fn delete(&self, key: Slice) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move { self.delete_sync(key) })
    }
}
