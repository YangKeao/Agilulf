extern crate agilulf;
extern crate env_logger;

use agilulf::{MemDatabase, Server};

fn main() {
    env_logger::init();

    let database = MemDatabase::default();
    let server = Server::new("127.0.0.1:3421", database).unwrap();

    server.run().unwrap();
}
