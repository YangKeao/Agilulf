//! This crate contains the most part of serialize and deserialize [Agilulf KV Protocol](https://github.com/YangKeao/Agilulf/blob/master/agilulf_protocol/README.md)
//!
//! The core function of this crate is provided by these methods:
//!
//! 1. `into_command_stream` in `request.rs`. This will convert a `TcpStream` into a `Stream<Command>`
//!
//! 2. `into_reply_sink` in `reply.rs`. This will convert a `TcpSink` into a `Sink<Reply>`
//!
//! 3. The implementation of `Into<Vec<u8>>` for `Command` and `Reply`. As this crate provides both
//! serialization and deserialization for both client and server. So they are all needed.

#![feature(async_await)]
#![feature(test)]
#![allow(clippy::needless_lifetimes)]
#![feature(type_alias_enum_variants)]

#[macro_use]
extern crate quick_error;

extern crate test;

mod async_buffer;
mod error;
mod message;
pub mod reply;
pub mod request;
mod slice;

pub use slice::Slice;

pub use reply::{Reply, Status};
pub use request::{Command, DeleteCommand, GetCommand, PutCommand, ScanCommand};

pub use error::database_error::{DatabaseError, Result as DatabaseResult};
pub use error::protocol_error::{ProtocolError, Result};

pub use async_buffer::{AsyncReadBuffer, AsyncWriteBuffer};
