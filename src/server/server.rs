use std::net::SocketAddr;

use futures::executor::{self, ThreadPool};
use futures::{StreamExt, AsyncReadExt, AsyncWriteExt};
use futures::task::{SpawnExt};

use romio::{TcpListener, TcpStream};

use log::{info};

use super::error::{Result};
use super::protocol::tcp_buffer::TcpStreamBuffer;
use super::protocol::{read_command, send_reply};
use super::protocol::ProtocolError;

use super::Command;
use crate::storage::Database;
use std::sync::Arc;

pub struct Server {
    addr: SocketAddr,
    listener: TcpListener,
    database: Arc<dyn Database>,
}

impl Server {
    pub fn new(address: &str, database: impl Database + 'static) -> Result<Server> {
        let addr = address.parse::<SocketAddr>()?;
        let listener = TcpListener::bind(&addr)?;

        Ok(Server { addr, listener, database: Arc::new(database) })
    }

    pub fn run(mut self) -> Result<()>{
        executor::block_on(async {
            let mut thread_pool = ThreadPool::new().unwrap(); // TODO: handler error here

            let mut incoming = self.listener.incoming();
            while let Some(stream) = incoming.next().await {
                let mut stream: TcpStream = stream.unwrap();

                let database = self.database.clone();
                thread_pool.spawn(async move {
                    handle_stream(stream, database).await;
                }).unwrap(); // TODO: handler error here
            }
        });

        Ok(())
    }
}

async fn handle_stream(stream: TcpStream, database: Arc<dyn Database>) -> Result<()> {
    let remote_addr = stream.peer_addr()?; // TODO: handler error here
    info!("Accepting stream from: {}", remote_addr);

    let mut stream_buffer = TcpStreamBuffer::new(stream);
    loop {
        match read_command(&mut stream_buffer).await {
            Ok(command) => {
                match command {
                    Command::GET(command) => {
                        info!("GET command received");
                        send_reply(&mut stream_buffer, database.get(command.key).into()).await.unwrap(); // TODO: handle this error
                        info!("GET reply sent");
                    }
                    Command::PUT(command) => {
                        info!("PUT command received");
                        send_reply(&mut stream_buffer, database.put(command.key, command.value).into()).await.unwrap();
                        info!("PUT reply sent");
                    }
                    Command::SCAN(command) => {
                        info!("SCAN command received");
                        send_reply(&mut stream_buffer, database.scan(command.start, command.end).into()).await.unwrap();
                        info!("SCAN reply sent");
                    }
                    _ => {}
                }
            }
            Err(err) => {
                match err {
                    ProtocolError::ConnectionClosed => {
                        // Connection closed
                        break;
                    }
                    _ => {
                        println!("{:?}", err);
                        // TODO: handle error here
                    }
                }
            }
        }
    }

    info!("Closing stream from: {}", remote_addr);
    Ok(())
}
