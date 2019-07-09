#![feature(async_await)]

#[macro_use]
extern crate quick_error;

pub mod error;
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
