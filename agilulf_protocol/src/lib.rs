#![feature(async_await)]
#![feature(test)]
#![allow(clippy::needless_lifetimes)]
#![feature(type_alias_enum_variants)]

#[macro_use]
extern crate quick_error;

extern crate test;

pub mod error;
mod message;
pub mod reply;
pub mod request;

mod async_buffer;
mod slice;

pub use reply::*;
pub use request::*;

pub use error::protocol_error::{ProtocolError, Result};
pub use slice::Slice;

pub use async_buffer::{AsyncReadBuffer, AsyncWriteBuffer};
use error::database_error::Result as DatabaseResult;
