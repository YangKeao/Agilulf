use std::net::SocketAddr;

use futures::executor::{self, ThreadPool};
use futures::{StreamExt, SinkExt};
use futures::task::{SpawnExt};
use futures::io::AsyncReadExt;

use romio::{TcpListener, TcpStream};

use log::{info};

use super::error::{Result};
use agilulf_protocol::{AsyncReadBuffer, AsyncWriteBuffer};
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
    let mut command_stream = AsyncReadBuffer::new(reader).into_command_stream();
    let mut write_buffer = AsyncWriteBuffer::new(writer).into_reply_sink();
    loop {
        let command = command_stream.next().await.unwrap();
        match command {
            Ok(command) => {
                match command {
                    Command::GET(command) => {
                        info!("GET {:?}", command.key.0.as_slice());
                        match write_buffer.send(database.get(command.key).await.into()).await {
                            _ => {}
                        } // TODO: handle error here
                        info!("GET reply sent");
                    }
                    Command::PUT(command) => {
                        info!("PUT {:?} {:?}", command.key.0.as_slice(), command.value.0.as_slice());
                        match write_buffer.send(database.put(command.key, command.value).await.into()).await {
                            _ => {}
                        }
                        info!("PUT reply sent");
                    }
                    Command::SCAN(command) => {
                        info!("SCAN command received");
                        match write_buffer.send(database.scan(command.start, command.end).await.into()).await {
                            _ => {}
                        }
                        info!("SCAN reply sent");
                    }
                    Command::DELETE(command) => {
                        info!("DELETE command received");
                        match write_buffer.send( database.delete(command.key).await.into()).await {
                            _ => {}
                        }
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
