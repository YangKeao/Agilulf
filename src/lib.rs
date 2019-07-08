#![feature(async_await)]

#[macro_use]
extern crate quick_error;

mod server;
mod storage;

pub use server::*;
pub use storage::{Database, Slice};
