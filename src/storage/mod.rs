pub mod error;
pub mod mem_database;
mod slice;

pub use slice::Slice;

use error::Result;

pub trait Database: Send + Sync {
    fn get(&self, key: Slice) -> Result<Slice>;
    fn put(&self, key: Slice, value: Slice) -> Result<()>;
    fn scan(&self, start: Slice, end: Slice) -> Result<Vec<Slice>>;
    fn delete(&self, key: Slice) -> Result<()>;
}
