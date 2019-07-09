#![feature(async_await)]
#![allow(clippy::needless_lifetimes)]

#[macro_use]
extern crate quick_error;

mod client;
mod error;

pub use client::AgilulfClient;
