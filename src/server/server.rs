use std::net::SocketAddr;

use futures::executor::{self, ThreadPool};
use futures::{StreamExt, AsyncReadExt, AsyncWriteExt};
use futures::task::{SpawnExt};

use romio::{TcpListener, TcpStream};

use log::{info};

use super::error::{Result};
use super::protocol::tcp_buffer::TcpStreamBuffer;
use super::protocol::read_message;
use super::protocol::ProtocolError;

pub struct Server {
    addr: SocketAddr,
    listener: TcpListener,
}

async fn handle_stream(stream: TcpStream) -> Result<()> {
    let remote_addr = stream.peer_addr()?; // TODO: handler error here
    info!("Accepting stream from: {}", remote_addr);

    let mut stream_buffer = TcpStreamBuffer::new(stream);
    loop {
        match read_message(&mut stream_buffer).await {
            Ok(message) => {
                println!("{:?}", message);
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

impl Server {
    pub fn new(address: &str) -> Result<Server> {
        let addr = address.parse::<SocketAddr>()?;
        let listener = TcpListener::bind(&addr)?;

        Ok(Server { addr, listener })
    }

    pub fn run(mut self) -> Result<()>{
        executor::block_on(async {
            let mut thread_pool = ThreadPool::new().unwrap(); // TODO: handler error here

            let mut incoming = self.listener.incoming();
            while let Some(stream) = incoming.next().await {
                let mut stream: TcpStream = stream.unwrap();

                thread_pool.spawn(async move {
                    handle_stream(stream).await;
                }).unwrap(); // TODO: handler error here
            }
        });

        Ok(())
    }
}
