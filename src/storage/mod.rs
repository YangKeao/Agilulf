pub mod mem_database;
use agilulf_protocol::Slice;

use agilulf_protocol::error::database_error::Result;
use futures::Future;
use std::pin::Pin;

pub trait Database: Send + Sync {
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
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Slice>>> + Send + '_>>;

    fn delete(&self, key: Slice) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
}
