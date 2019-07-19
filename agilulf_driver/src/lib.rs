//! This is a client of Agilulf KV Server.
//!
//! Many performance optimization will be done in the future. But now it is simple and fast enough.
//!
//! The bench in tests of this crate uses `MemDatabase` to avoid the influence of database algorithm.
//! But as `MemDatabase` is simply implemented by `SkipList`, it will cost a lot of RAM.
//!
//! Two structs are provided to connect with a server:
//!
//! 1. `AgilulfClient`: The basic client, it will send request and receive response and then return
//! the function (actually return `Poll::Ready`). Batch send strategy is quite simple: `send_all`.
//! The response order is kept consistent with request order.
//!
//! 2. `MultiAgilulfClient`: A client can use multiple TCP Stream. Use multiple TCP Stream at the same
//! time can increase the performance quite well. A `MultiAgilulfClient` will open multiple `AgilulfClient`
//! and give them an ID. Any request will be hashed by key and send to corresponding client. As it is a
//! KV server, just keeping operation order consistent of the same key is enough. **Note**: `SCAN` is
//! different, this method will affect multiple keys. Guard (just like CPU's Memory Guard) is needed
//! for this operation.

#![feature(async_await)]
#![feature(async_closure)]
#![feature(test)]
#![allow(clippy::needless_lifetimes)]

extern crate test;

#[macro_use]
extern crate quick_error;

mod client;
mod error;

pub use client::{AgilulfClient, MultiAgilulfClient};
