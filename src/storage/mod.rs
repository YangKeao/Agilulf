pub mod mem_database;
use agilulf_protocol::Slice;

use agilulf_protocol::error::database_error::Result;

pub trait Database: Send + Sync {
    fn get(&self, key: Slice) -> Result<Slice>;
    fn put(&self, key: Slice, value: Slice) -> Result<()>;
    fn scan(&self, start: Slice, end: Slice) -> Result<Vec<Slice>>;
    fn delete(&self, key: Slice) -> Result<()>;
}
