pub mod error;
pub mod mem_database;

use error::Result;
use std::cmp::Ordering;

use libc::memcmp;

#[derive(Clone, Eq)]
pub struct Slice(pub Vec<u8>);

impl PartialOrd for Slice {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.0.len() < other.0.len() {
            return Some(Ordering::Less);
        } else {
            let res = unsafe {
                memcmp(
                    self.0.as_ptr() as *const core::ffi::c_void,
                    other.0.as_ptr() as *const core::ffi::c_void,
                    self.0.len(),
                )
            };
            if res == 0 {
                return Some(Ordering::Equal);
            } else if res < 0 {
                return Some(Ordering::Less);
            } else {
                return Some(Ordering::Greater);
            }
        }
    }
}

impl PartialEq for Slice {
    fn eq(&self, other: &Self) -> bool {
        self.partial_cmp(other) == Some(Ordering::Equal)
    }
}

impl Ord for Slice {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap() // here it is safe.
    }
}

pub trait Database: Send + Sync {
    fn get(&self, key: Slice) -> Result<Slice>;
    fn put(&self, key: Slice, value: Slice) -> Result<()>;
    fn scan(&self, start: Slice, end: Slice) -> Result<Vec<Slice>>;
}
