#![feature(async_await)]
#![feature(test)]
#![allow(clippy::needless_lifetimes)]

#[macro_use]
extern crate quick_error;

extern crate test;

pub mod error;
mod message;
pub mod reply;
pub mod request;

mod slice;
mod tcp_buffer;

pub use reply::*;
pub use request::*;

pub use error::protocol_error::{ProtocolError, Result};
pub use slice::Slice;

use error::database_error::Result as DatabaseResult;
pub use tcp_buffer::TcpStreamBuffer;
