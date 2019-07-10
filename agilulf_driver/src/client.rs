use std::net::SocketAddr;

use agilulf_protocol::{read_reply, Command, DeleteCommand, GetCommand, PutCommand, Reply, ScanCommand, Slice, AsyncReadBuffer, AsyncWriteBuffer, ProtocolError};
use romio::TcpStream;

use super::error::{Result};
use futures::io::{AsyncReadExt, WriteHalf};
use futures::channel::mpsc::{self, UnboundedReceiver};
use futures::{SinkExt, StreamExt};
use futures::lock::Mutex;

pub struct AgilulfClient {
    reply_receiver: UnboundedReceiver<std::result::Result<Reply, ProtocolError>>,
    write_stream: AsyncWriteBuffer<WriteHalf<TcpStream>>,
}

impl AgilulfClient {
    pub async fn connect(address: &str) -> Result<AgilulfClient> {
        let addr = address.parse::<SocketAddr>()?;
        let stream = TcpStream::connect(&addr).await?;
        let (reader, writer) = stream.split();
        let write_stream = AsyncWriteBuffer::new(writer);

        let (mut reply_sender, reply_receiver) = mpsc::unbounded();
        std::thread::spawn(move || {
            let reply_future = async move {
                let mut reader = AsyncReadBuffer::new(reader);
                loop  {
                    let reply = read_reply(&mut reader).await;
                    reply_sender.send(reply).await.unwrap(); // TODO: handle error here
                }
            };
            futures::executor::block_on(reply_future);
        });

        Ok(AgilulfClient {
            reply_receiver,
            write_stream,
        })
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

        self.write_stream.write_all(message).await?;

        self.read_reply().await
    }

    pub async fn send_batch(&mut self, commands: Vec<Command>) -> Result<Vec<Reply>> {
        let len = commands.len();
        let mut messages = Vec::new();

        for command in commands {
            let mut message: Vec<u8> = command.into();
            messages.append(&mut message);
        }

        self.write_stream.write_all(messages).await?;

        let mut replies = Vec::new();
        for _ in 0..len {
            replies.push(self.read_reply().await?)
        }
        Ok(replies)
    }

    pub async fn read_reply(&mut self) -> Result<Reply> {
        Ok(self.reply_receiver.select_next_some().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agilulf_protocol::Status;
    use agilulf::{MemDatabase, Server};
    use std::sync::{Once, Arc};
    use std::sync::atomic::{AtomicI16, Ordering};
    use futures::executor::ThreadPool;
    use futures::task::{SpawnExt};
    use futures::Future;
    use rand::{Rng, thread_rng};
    use rand::distributions::Standard;

    static INIT: Once = Once::new();
    static SERVER_PORT: AtomicI16 = AtomicI16::new(7000);

    fn init() {
        INIT.call_once(|| {
            env_logger::init();
        });
    }

    fn setup_server(executor: &mut ThreadPool) -> i16 {
        init();
        let server_port = SERVER_PORT.fetch_add(1, Ordering::Relaxed);
        let address = format!("127.0.0.1:{}", server_port);

        let database = MemDatabase::default();
        let server = Server::new(address.as_str(), database).unwrap();

        executor.spawn(server.run_async()).unwrap();

        return server_port;
    }

    async fn connect(server_port: i16) -> AgilulfClient {
        let address = format!("127.0.0.1:{}", server_port);
        loop {
            match AgilulfClient::connect(address.as_str()).await {
                Err(_) => {},
                Ok(client) => return client,
            }
        }
    }

    fn run_test<F, Fut >(f: F)
        where F: FnOnce(AgilulfClient, ThreadPool) -> Fut + Send + Sync,
              Fut: Future<Output=()> + Send {
        let mut thread_pool = ThreadPool::builder()
            .name_prefix("test_thread").create().unwrap();

        let port = setup_server(&mut thread_pool);

        let extra_thread_pool = thread_pool.clone();

        thread_pool.run(async move {
            let client = connect(port.clone()).await;
            f(client, extra_thread_pool).await;
        });
    }

    #[test]
    fn put_get_test() {
        run_test(async move |mut client, mut thread_pool| {
            for i in 0..100 {
                let ans = client.put(Slice(format!("key{}", i).into_bytes()), Slice(format!("value{}", i).into_bytes())).await.unwrap();
                assert_eq!(ans, Reply::StatusReply(Status::OK));
            }

            for i in 0..100 {
                let ans = client.get(Slice(format!("key{}", i).into_bytes())).await.unwrap();
                assert_eq!(ans, Reply::SliceReply(Slice(format!("value{}", i).into_bytes())));
            }
        });
    }

    #[test]
    fn put_delete_get_test() {
        run_test(async move |mut client, mut thread_pool| {
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
        });
    }

    #[test]
    fn override_test() {
        run_test(async move |mut client, mut thread_pool| {
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
        });
    }

    fn generate_request(num: usize) -> Vec<Command> {
        let keys: Vec<Vec<u8>> = (0..num).map(|_| {
            thread_rng().sample_iter(&Standard).take(8).collect()
        }).collect();

        let value: Vec<Vec<u8>> = (0..num).map(|_| {
            thread_rng().sample_iter(&Standard).take(256).collect()
        }).collect();

        (0..num).map(|index| {
            Command::PUT(PutCommand {
                key: Slice(keys[index].clone()),
                value: Slice(value[index].clone()),
            })
        }).collect()
    }

    #[test]
    fn batch_put_request() {
        let requests = generate_request(1000);
        let requests = &requests;

        run_test(async move |mut client, mut thread_pool| {
            let replies = client.send_batch(requests.to_vec()).await.unwrap();
            for reply in replies {
                assert_eq!(reply, Reply::StatusReply(Status::OK))
            }
        });
    }

    #[bench]
    fn single_thread_bench(b: &mut test::Bencher) {
        let requests = generate_request(1000);
        let requests = &requests;

        run_test(async move |mut client, mut thread_pool| {
            let client = &client;
            b.iter(async move || {
                client.send_batch(requests.to_vec()).await.unwrap();
            });
        });
    }
}
