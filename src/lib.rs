//! A simple but fast KV database (like ![LevelDB](https://github.com/google/leveldb))
//!
//! This crate provide an abstraction layer of AsyncDatabase. User can select which database to use
//! easily. Any struct implements [AsyncDatabase](./trait.AsyncDatabase.html) trait can construct a
//! TCP server.
//!
//! It actually also provides some implementation of it:
//!
//! - [Database](./struct.Database.html) stores data as LSM structure in disk.
//!
//! - [MemDatabase](./struct.MemDatabase.html) uses a lock-free skiplist to store data in Memory.
//! Note: with the increasing of the data size, it will become slower and slower.
//!
//! [TCP Server](./struct.Server.html) is built with [romio](struct.Server.html) and
//! [futures-preview](https://github.com/rust-lang-nursery/futures-rs). It spawns every connected tcp
//! stream on a ThreadPool.
//!

#![feature(async_await)]
#![feature(test)]
#![feature(box_syntax)]
#![feature(atomic_min_max)]
#![allow(clippy::needless_lifetimes)]

extern crate test;

#[macro_use]
extern crate quick_error;

mod log;
mod server;
mod storage;

pub use server::Server;
pub use storage::mem_database::MemDatabase;
pub use storage::{AsyncDatabase, SyncDatabase};
pub use storage::{Database, DatabaseBuilder};
