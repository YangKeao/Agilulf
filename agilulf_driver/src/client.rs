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
    use std::sync::atomic::{AtomicI16, Ordering};

    static INIT: Once = Once::new();
    static SERVER_PORT: AtomicI16 = AtomicI16::new(7000);

    fn init() {
        INIT.call_once(|| {
            env_logger::init();
        });
    }

    async fn setup() -> AgilulfClient {
        init();
        let server_port = SERVER_PORT.fetch_add(1, Ordering::Relaxed);
        let address = format!("127.0.0.1:{}", server_port);

        let cloned_address = address.clone();
        std::thread::spawn(move || {
            let database = MemDatabase::new();
            let server = Server::new(cloned_address.as_str(), database).unwrap();

            server.run().unwrap();
        });

        while let Err(e) = AgilulfClient::new(address.as_str()).await {}
        AgilulfClient::new(address.as_str()).await.unwrap()
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

    #[test]
    fn put_delete_get_test() {
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

            for i in 0..100 {
                if i%2 == 0 {
                    let ans = client.delete(Slice(format!("key{}", i).into_bytes())).await.unwrap();
                    assert_eq!(ans, Reply::StatusReply(Status::OK));
                }
            }

            for i in 0..100 {
                let ans = client.get(Slice(format!("key{}", i).into_bytes())).await.unwrap();
                if i % 2 == 0 {
                    assert_eq!(ans, Reply::ErrorReply(String::from("KeyNotFound\r\n")));
                } else {
                    assert_eq!(ans, Reply::SliceReply(Slice(format!("value{}", i).into_bytes())));
                }
            }
        };
        futures::executor::block_on(future);
    }

    #[test]
    fn override_test() {
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

            for i in 0..100 {
                if i%2 == 0 {
                    let ans = client.put(Slice(format!("key{}", i).into_bytes()), Slice(format!("new_value{}", i).into_bytes())).await.unwrap();
                    assert_eq!(ans, Reply::StatusReply(Status::OK));
                }
            }

            for i in 0..100 {
                let ans = client.get(Slice(format!("key{}", i).into_bytes())).await.unwrap();
                if i % 2 == 0 {
                    assert_eq!(ans, Reply::SliceReply(Slice(format!("new_value{}", i).into_bytes())));
                } else {
                    assert_eq!(ans, Reply::SliceReply(Slice(format!("value{}", i).into_bytes())));
                }
            }
        };
        futures::executor::block_on(future);
    }
}
