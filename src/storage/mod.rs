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

/// Abstraction layer for a SyncDatabase. Every method should return directly.
pub trait SyncDatabase: Send + Sync {
    fn get_sync(&self, key: Slice) -> Result<Slice>;

    fn put_sync(&self, key: Slice, value: Slice) -> Result<()>;

    fn scan_sync(&self, start: Slice, end: Slice) -> Vec<(Slice, Slice)>;

    fn delete_sync(&self, key: Slice) -> Result<()>;
}

/// Abstraction layer for a AsyncDatabase. Every method return a Future.
///
/// The return type of these function are fixed as `Pin<Box<dyn Future<Output = Result<_>> + Send + '_>>`
/// rather than a generic type for convenience.
pub trait AsyncDatabase: Send + Sync {
    fn get(&self, key: Slice) -> Pin<Box<dyn Future<Output = Result<Slice>> + Send + '_>>;

    fn put(
        &self,
        key: Slice,
        value: Slice,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// SCAN don't return a `Result` because if nothing is found, an empty vector will be returned.
    fn scan(
        &self,
        start: Slice,
        end: Slice,
    ) -> Pin<Box<dyn Future<Output = Vec<(Slice, Slice)>> + Send + '_>>;

    fn delete(&self, key: Slice) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
}

/// Every sync database can be wrapped as an async database easily. With this wrapper, MemDatabase can
/// be used directly on Server.
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
