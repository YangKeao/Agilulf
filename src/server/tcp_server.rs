use std::net::SocketAddr;

use futures::executor::{self, ThreadPool};
use futures::{StreamExt};
use futures::task::{SpawnExt};
use futures::io::AsyncReadExt;

use romio::{TcpListener, TcpStream};

use log::{info};

use super::error::{Result};
use agilulf_protocol::{AsyncReadBuffer, AsyncWriteBuffer};
use agilulf_protocol::{read_command, send_reply};
use agilulf_protocol::ProtocolError;

use agilulf_protocol::Command;
use crate::storage::Database;
use std::sync::Arc;

pub struct Server {
    listener: TcpListener,
    database: Arc<dyn Database>,
}

impl Server {
    pub fn new(address: &str, database: impl Database + 'static) -> Result<Server> {
        let addr = address.parse::<SocketAddr>()?;
        let listener = TcpListener::bind(&addr)?;

        Ok(Server { listener, database: Arc::new(database) })
    }

    pub async fn run_async(mut self) {
        let mut thread_pool = ThreadPool::new().unwrap(); // TODO: handler error here

        let mut incoming = self.listener.incoming();
        while let Some(stream) = incoming.next().await {
            let stream: TcpStream = stream.unwrap();

            let database = self.database.clone();
            thread_pool.spawn(async move {
                handle_stream(stream, database).await.unwrap(); // TODO: handle error here
            }).unwrap(); // TODO: handler error here
        }
    }

    pub fn run(self) -> Result<()>{
        executor::block_on(async {
            self.run_async().await
        });

        Ok(())
    }
}

async fn handle_stream(stream: TcpStream, database: Arc<dyn Database>) -> Result<()> {
    let remote_addr = stream.peer_addr()?; // TODO: handle error here
    info!("Accepting stream from: {}", remote_addr);

    let (reader, writer) = stream.split();
    let mut read_buffer = AsyncReadBuffer::new(reader);
    let mut write_buffer = AsyncWriteBuffer::new(writer);
    loop {
        match read_command(&mut read_buffer).await {
            Ok(command) => {
                match command {
                    Command::GET(command) => {
                        info!("GET {:?}", command.key.0.as_slice());
                        send_reply(&mut write_buffer, database.get(command.key).into()).await.unwrap(); // TODO: handle this error
                        info!("GET reply sent");
                    }
                    Command::PUT(command) => {
                        info!("PUT {:?} {:?}", command.key.0.as_slice(), command.value.0.as_slice());
                        send_reply(&mut write_buffer, database.put(command.key, command.value).into()).await.unwrap();
                        info!("PUT reply sent");
                    }
                    Command::SCAN(command) => {
                        info!("SCAN command received");
                        send_reply(&mut write_buffer, database.scan(command.start, command.end).into()).await.unwrap();
                        info!("SCAN reply sent");
                    }
                    Command::DELETE(command) => {
                        info!("DELETE command received");
                        send_reply(&mut write_buffer, database.delete(command.key).into()).await.unwrap();
                        info!("DELETE reply sent");
                    }
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
