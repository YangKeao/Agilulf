extern crate agilulf;
extern crate env_logger;

use agilulf::{Server, Slice};
use std::collections::BTreeMap;
use std::sync::RwLock;

fn main() {
    env_logger::init();

    let database: RwLock<BTreeMap<Slice, Slice>> = RwLock::new(BTreeMap::new());
    let server = Server::new("127.0.0.1:3421", database).unwrap();

    server.run();
}
