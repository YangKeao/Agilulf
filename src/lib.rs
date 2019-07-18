#![feature(async_await)]
#![feature(type_alias_enum_variants)]
#![feature(test)]
#![feature(box_syntax)]
#![allow(clippy::needless_lifetimes)]

extern crate test;

#[macro_use]
extern crate quick_error;

mod log;
mod server;
mod storage;

pub use agilulf_protocol::Slice;
pub use server::*;
pub use storage::mem_database::MemDatabase;
pub use storage::AsyncDatabase;
pub use storage::{Database, DatabaseBuilder};
