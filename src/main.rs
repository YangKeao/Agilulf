extern crate agilulf;
extern crate env_logger;

use agilulf::{MemDatabase, Server, Slice};
use std::collections::BTreeMap;
use std::sync::RwLock;

fn main() {
    env_logger::init();

    let database = MemDatabase::new();
    let server = Server::new("127.0.0.1:3421", database).unwrap();

    server.run();
}
