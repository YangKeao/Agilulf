use std::net::SocketAddr;

use futures::executor::{self, ThreadPool};
use futures::io::AsyncReadExt;
use futures::task::SpawnExt;
use futures::{SinkExt, StreamExt};

use romio::{TcpListener, TcpStream};

use log::info;

use super::error::Result;
use agilulf_protocol::Result as ProtocolResult;
use agilulf_protocol::{AsyncReadBuffer, AsyncWriteBuffer};

use crate::storage::AsyncDatabase;
use agilulf_protocol::Command;
use std::sync::Arc;

pub struct Server {
    listener: TcpListener,
    database: Arc<dyn AsyncDatabase>,
}

impl Server {
    pub fn new(address: &str, database: impl AsyncDatabase + 'static) -> Result<Server> {
        let addr = address.parse::<SocketAddr>()?;
        let listener = TcpListener::bind(&addr)?;

        Ok(Server {
            listener,
            database: Arc::new(database),
        })
    }

    pub async fn run_async(mut self) -> Result<()> {
        let mut thread_pool = ThreadPool::new()?;

        let mut incoming = self.listener.incoming();
        while let Some(stream) = incoming.next().await {
            let stream: TcpStream = stream.unwrap();

            let database = self.database.clone();
            thread_pool.spawn(async move {
                match handle_stream(stream, database).await {
                    Ok(()) => {}
                    Err(err) => log::error!("Error while handling stream: {}", err),
                }
            })?
        }
        Ok(())
    }

    pub fn run(self) -> Result<()> {
        executor::block_on(async { self.run_async().await });

        Ok(())
    }
}

async fn handle_stream(stream: TcpStream, database: Arc<dyn AsyncDatabase>) -> Result<()> {
    let remote_addr = stream.peer_addr()?;
    info!("Accepting stream from: {}", remote_addr);

    let (reader, writer) = stream.split();
    let mut command_stream = AsyncReadBuffer::new(reader).into_command_stream().fuse();
    let reply_sink = AsyncWriteBuffer::new(writer).into_reply_sink();

    let mut process_sink = reply_sink.with(|command: ProtocolResult<Command>| {
        Box::pin(async {
            match command {
                Ok(command) => match command {
                    Command::GET(command) => {
                        ProtocolResult::Ok(database.get(command.key).await.into())
                    }
                    Command::PUT(command) => {
                        ProtocolResult::Ok(database.put(command.key, command.value).await.into())
                    }
                    Command::SCAN(command) => {
                        ProtocolResult::Ok(database.scan(command.start, command.end).await.into())
                    }
                    Command::DELETE(command) => {
                        ProtocolResult::Ok(database.delete(command.key).await.into())
                    }
                },
                Err(err) => ProtocolResult::Err(err),
            }
        })
    });

    loop {
        let command = command_stream.select_next_some().await;
        if let Err(e) = process_sink.send(command).await {
            println!("{:?}", e);
            break;
        }
    }

    info!("Closing stream from: {}", remote_addr);
    Ok(())
}
