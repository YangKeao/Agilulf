use std::net::SocketAddr;

use agilulf_protocol::{
    read_reply, Command, DeleteCommand, GetCommand, PutCommand, Reply, ScanCommand, Slice,
    TcpStreamBuffer,
};
use romio::TcpStream;

use super::error::{Result};
use futures::io::AsyncWriteExt;

pub struct AgilulfClient {
    stream: TcpStreamBuffer,
}

impl AgilulfClient {
    pub async fn new(address: &str) -> Result<AgilulfClient> {
        let addr = address.parse::<SocketAddr>()?;
        let stream = TcpStream::connect(&addr).await?;
        let stream = TcpStreamBuffer::new(stream);

        Ok(AgilulfClient { stream })
    }

    pub async fn put(&mut self, key: Slice, value: Slice) -> Result<Reply> {
        self.send(Command::PUT(PutCommand { key, value })).await
    }

    pub async fn get(&mut self, key: Slice) -> Result<Reply> {
        self.send(Command::GET(GetCommand { key })).await
    }

    pub async fn delete(&mut self, key: Slice) -> Result<Reply> {
        self.send(Command::DELETE(DeleteCommand { key })).await
    }

    pub async fn scan(&mut self, start: Slice, end: Slice) -> Result<Reply> {
        self.send(Command::SCAN(ScanCommand { start, end })).await
    }

    pub async fn send(&mut self, command: Command) -> Result<Reply> {
        let message: Vec<u8> = command.into();
        println!("{:?}", std::str::from_utf8(message.as_slice()).unwrap());

        self.stream.write_all(message).await;

        self.read_reply().await
    }

    pub async fn read_reply(&mut self) -> Result<Reply> {
        Ok(read_reply(&mut self.stream).await?)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use agilulf_protocol::Status;
    use agilulf::{MemDatabase, Server};
    use std::sync::Once;
    use std::thread::JoinHandle;

    const ADDRESS: &str = "127.0.0.1:3421";

    static START_SERVER: Once = Once::new();

    fn start_server() {
        START_SERVER.call_once(|| {
            std::thread::spawn(|| {
                let database = MemDatabase::new();
                let server = Server::new(ADDRESS, database).unwrap();

                server.run().unwrap();
            });
        });

    }

    async fn setup() -> AgilulfClient {
        start_server();
        while let Err(e) = AgilulfClient::new("127.0.0.1:3421").await {}
        AgilulfClient::new("127.0.0.1:3421").await.unwrap()
    }

    #[test]
    fn put_get_test() {
        let future = async {
            let mut client = setup().await;

            for i in 0..100 {
                let ans = client.put(Slice(format!("key{}", i).into_bytes()), Slice(format!("value{}", i).into_bytes())).await.unwrap();
                assert_eq!(ans, Reply::StatusReply(Status::OK));
            }

            for i in 0..100 {
                let ans = client.get(Slice(format!("key{}", i).into_bytes())).await.unwrap();
                assert_eq!(ans, Reply::SliceReply(Slice(format!("value{}", i).into_bytes())));
            }
        };
        futures::executor::block_on(future);
    }
}
