#![feature(async_await)]
#![feature(test)]
#![allow(clippy::needless_lifetimes)]

extern crate test;

#[macro_use]
extern crate quick_error;

mod client;
mod error;

pub use client::AgilulfClient;
