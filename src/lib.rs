#![feature(async_await)]
#![allow(clippy::needless_lifetimes)]

#[macro_use]
extern crate quick_error;

mod server;
mod storage;

pub use agilulf_protocol::Slice;
pub use server::*;
pub use storage::mem_database::MemDatabase;
pub use storage::Database;
