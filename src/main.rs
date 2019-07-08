extern crate agilulf;
extern crate env_logger;

use agilulf::Server;

fn main() {
    env_logger::init();

    let server = Server::new("127.0.0.1:3421").unwrap();

    server.run();
}
