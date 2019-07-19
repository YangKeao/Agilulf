#![feature(box_syntax)]

extern crate agilulf;
extern crate clap;
extern crate env_logger;
extern crate log;

use agilulf::{DatabaseBuilder, MemDatabase, Server};
use clap::{App, Arg};

fn main() {
    env_logger::init();

    let matches = App::new("Agilulf")
        .version("0.1.0")
        .author("Yang Keao <keao.yang@yahoo.com>")
        .about("A concurrent KV database")
        .arg(
            Arg::with_name("mem")
                .long("mem")
                .help("Run memory only database"),
        )
        .arg(
            Arg::with_name("base_dir")
                .long("base_dir")
                .value_name("BASE_DIR")
                .default_value("/var/tmp/agilulf")
                .help("Set the base directory of database")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("forget")
                .long("forget")
                .help("Ignore the existed log and sstables"),
        )
        .arg(
            Arg::with_name("addr")
                .long("addr")
                .value_name("ADDR")
                .default_value("127.0.0.1:3421")
                .help("Set the listening address of the database")
                .takes_value(true),
        )
        .get_matches();

    let address = matches.value_of("addr").unwrap_or("127.0.0.1:3421");
    let server = match matches.is_present("mem") {
        true => {
            log::info!("Build memory database");
            Server::new(address, MemDatabase::default())
        }
        false => {
            log::info!("Build database");
            let mut builder = DatabaseBuilder::default();
            builder
                .base_dir(
                    matches
                        .value_of("BASE_DIR")
                        .unwrap_or("/var/tmp/agilulf")
                        .to_string(),
                )
                .restore(!matches.is_present("forget"));

            match builder.build() {
                Ok(db) => Server::new(address, db),
                Err(err) => {
                    println!("Error occurred during building database: {:?}", err);
                    return;
                }
            }
        }
    };
    let server = match server {
        Ok(server) => server,
        Err(err) => {
            println!("Error occurred during start up server: {:?}", err);
            return;
        }
    };

    log::info!("Now starting server");
    match server.run() {
        Ok(()) => {}
        Err(err) => {
            println!("LocalPool spawn task error: {:?}", err);
        }
    };
}
