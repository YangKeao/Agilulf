use std::net::SocketAddr;

use futures::executor::{self, ThreadPool};
use futures::io::AsyncReadExt;
use futures::task::SpawnExt;
use futures::{SinkExt, StreamExt};

use romio::{TcpListener, TcpStream};

use log::info;

use super::error::Result;
use agilulf_protocol::{AsyncReadBuffer, AsyncWriteBuffer};
use agilulf_protocol::{ProtocolError, Result as ProtocolResult};

use crate::storage::AsyncDatabase;
use agilulf_protocol::Command;
use std::sync::Arc;

/// A simple TCP server constructed by a foreign database with the help of `agilulf_protocol`
///
/// Most of it's coded are neccesary and template. There isn't much logic code in this mod:
///
/// First, set up a romio TCP server and spawn every connection on a ThreadPool.
///
/// For every connection, convert the Incoming stream into command stream and send the command to
/// database. Then read reply from database (actually not read, it is just function return). Send the
/// reply to reply sink (converted from outcoming sink)
///
/// This struct just assemble the protocol and database together. With this template, implement a
/// KV server on another transimission layer is quite the same. Just replace the romio TCP server with
/// other asynchronous server and most codes are same.
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

    /// `run_async` is provided as a seperated function. Becasue you may want to run on a ThreadPool
    /// or any other executor. I don't want to limit the executor choice.
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

    /// This function use `futures::executor::block_on` to run method `run_async`
    pub fn run(self) -> Result<()> {
        executor::block_on(async { self.run_async().await })?;

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
                Err(err) => ProtocolResult::Ok(err.into()),
            }
        })
    });

    loop {
        let command = command_stream.select_next_some().await;
        if let Err(err) = process_sink.send(command).await {
            match &err {
                ProtocolError::IOError(err) => match err.kind() {
                    std::io::ErrorKind::BrokenPipe => {
                        break;
                    }
                    _ => {}
                },
                _ => {}
            }
            log::error!("Error while sending reply {:?}", err);
            break;
        }
    }

    info!("Closing stream from: {}", remote_addr);
    Ok(())
}
