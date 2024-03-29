use std::net::SocketAddr;

use agilulf_protocol::{
    AsyncReadBuffer, AsyncWriteBuffer, Command, DeleteCommand, GetCommand, ProtocolError,
    PutCommand, Reply, ScanCommand, Slice,
};
use romio::TcpStream;

use super::error::Result;
use futures::channel::mpsc::{self, UnboundedReceiver};
use futures::io::{AsyncReadExt, WriteHalf};
use futures::lock::Mutex;
use futures::{SinkExt, StreamExt};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// A simple single-thread client
#[derive(Clone)]
pub struct AgilulfClient {
    reply_receiver: Arc<Mutex<UnboundedReceiver<std::result::Result<Reply, ProtocolError>>>>,
    write_stream: Arc<Mutex<AsyncWriteBuffer<WriteHalf<TcpStream>>>>,
}

impl AgilulfClient {
    pub async fn connect(address: &str) -> Result<AgilulfClient> {
        let addr = address.parse::<SocketAddr>()?;
        let stream = TcpStream::connect(&addr).await?;
        let (reader, writer) = stream.split();
        let write_stream = Arc::new(Mutex::new(AsyncWriteBuffer::new(writer)));

        let (mut reply_sender, reply_receiver) = mpsc::unbounded();
        let reply_receiver = Arc::new(Mutex::new(reply_receiver));

        std::thread::spawn(move || {
            let reply_future = async move {
                let mut reply_stream = AsyncReadBuffer::new(reader).into_reply_stream();
                reply_sender.send_all(&mut reply_stream).await
            };
            futures::executor::block_on(reply_future).unwrap(); // TODO: handle error here
        });

        Ok(AgilulfClient {
            reply_receiver,
            write_stream,
        })
    }

    pub async fn put(&self, key: Slice, value: Slice) -> Result<Reply> {
        self.send(Command::PUT(PutCommand { key, value })).await
    }

    pub async fn get(&self, key: Slice) -> Result<Reply> {
        self.send(Command::GET(GetCommand { key })).await
    }

    pub async fn delete(&self, key: Slice) -> Result<Reply> {
        self.send(Command::DELETE(DeleteCommand { key })).await
    }

    pub async fn scan(&self, start: Slice, end: Slice) -> Result<Reply> {
        self.send(Command::SCAN(ScanCommand { start, end })).await
    }

    pub async fn send(&self, command: Command) -> Result<Reply> {
        let message: Vec<u8> = command.into();

        let mut write_stream = self.write_stream.lock().await;
        write_stream.write_all(message).await?;

        self.read_reply().await
    }

    pub async fn send_batch(&self, commands: Vec<Command>) -> Result<Vec<Reply>> {
        let len = commands.len();
        let mut messages = Vec::new();

        for command in commands {
            let mut message: Vec<u8> = command.into();
            messages.append(&mut message);
        }

        let mut write_stream = self.write_stream.lock().await;
        write_stream.write_all(messages).await?;

        let mut replies = Vec::new();
        for _ in 0..len {
            replies.push(self.read_reply().await?)
        }
        Ok(replies)
    }

    pub async fn read_reply(&self) -> Result<Reply> {
        let mut receiver = self.reply_receiver.lock().await;
        Ok(receiver.select_next_some().await?)
    }
}

/// A multi-thread client.
///
/// A multi-thread client will open several single-thread clients (called knight) and give them an ID. Any request
/// will be hashed by key and send to corresponding client. As it is a KV server, just keeping operation
/// order consistent of the same key is enough. **Note**: `SCAN` is different, this method will
/// affect multiple keys. Guard (just like CPU's Memory Guard) is needed for this operation. Some
/// strategy can be choosed:
///
/// 1. Any request related before `SCAN` should have finished, then `SCAN`, then following requests.
///
/// 2. Allocate all requests related with `SCAN` into the same knight, then the order is consistent
/// natually
///
#[derive(Clone)]
pub struct MultiAgilulfClient {
    knights: Arc<Vec<AgilulfClient>>,
}

impl MultiAgilulfClient {
    pub async fn connect(address: &str, knights_num: usize) -> Result<MultiAgilulfClient> {
        let mut knights = Vec::new();
        for _ in 0..knights_num {
            knights.push(AgilulfClient::connect(address).await?)
        }
        let knights = Arc::new(knights);

        Ok(MultiAgilulfClient { knights })
    }
    fn hash_key(key: &Slice) -> usize {
        let mut hasher = fnv::FnvHasher::default();
        key.0.hash(&mut hasher);
        hasher.finish() as usize
    }
    pub fn allocate_task(&self, command: &Command) -> usize {
        match command {
            Command::PUT(command) => Self::hash_key(&command.key) % self.knights.len(),
            Command::DELETE(command) => Self::hash_key(&command.key) % self.knights.len(),
            Command::GET(command) => Self::hash_key(&command.key) % self.knights.len(),
            Command::SCAN(command) => {
                Self::hash_key(&command.start) % self.knights.len() // TODO: Add Barrier here
            }
        }
    }

    pub async fn put(&self, key: Slice, value: Slice) -> Result<Reply> {
        self.send(Command::PUT(PutCommand { key, value })).await
    }

    pub async fn get(&self, key: Slice) -> Result<Reply> {
        self.send(Command::GET(GetCommand { key })).await
    }

    pub async fn delete(&self, key: Slice) -> Result<Reply> {
        self.send(Command::DELETE(DeleteCommand { key })).await
    }

    pub async fn scan(&self, start: Slice, end: Slice) -> Result<Reply> {
        self.send(Command::SCAN(ScanCommand { start, end })).await // TODO: need barrier for safety of scan.
    }

    pub async fn send(&self, command: Command) -> Result<Reply> {
        let knight_id = self.allocate_task(&command);
        self.knights[knight_id].send(command).await
    }

    pub async fn send_batch(&self, commands: Vec<Command>) -> Result<Vec<Reply>> {
        let commands: Vec<(usize, &Command)> = commands
            .iter()
            .map(|command| (self.allocate_task(command), command))
            .collect();

        let mut futures = Vec::new();
        for knight_id in 0..self.knights.len() {
            futures.push(
                self.knights[knight_id].send_batch(
                    commands
                        .iter()
                        .filter_map(|(command_id, command)| {
                            if knight_id == *command_id {
                                Some((*command).clone())
                            } else {
                                None
                            }
                        })
                        .collect(),
                ),
            )
        }

        let mut future_replies = Vec::new();
        for reply in futures::future::join_all(futures).await {
            future_replies.push(reply?.into_iter());
        }

        let mut replies = Vec::new();
        for (id, _) in commands.iter() {
            match future_replies[*id].next() {
                Some(reply) => replies.push(reply),
                None => unreachable!(),
            }
        }
        Ok(replies)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agilulf::{MemDatabase, Server};
    use agilulf_protocol::Status;
    use futures::executor::ThreadPool;
    use futures::task::SpawnExt;
    use futures::Future;
    use rand::distributions::Standard;
    use rand::{thread_rng, Rng};
    use std::sync::atomic::{AtomicI16, Ordering};
    use std::sync::Once;

    static INIT: Once = Once::new();
    static SERVER_PORT: AtomicI16 = AtomicI16::new(7000);

    fn init() {
        INIT.call_once(|| {
            env_logger::init();
        });
    }

    fn setup_server() -> i16 {
        init();
        let server_port = SERVER_PORT.fetch_add(1, Ordering::Relaxed);
        let address = format!("127.0.0.1:{}", server_port);

        let database = MemDatabase::default();
        let server = Server::new(address.as_str(), database).unwrap();

        std::thread::Builder::new()
            .name(String::from("server_thread"))
            .spawn(|| {
                server.run().unwrap();
            })
            .unwrap();

        return server_port;
    }

    async fn connect(server_port: i16) -> AgilulfClient {
        let address = format!("127.0.0.1:{}", server_port);
        loop {
            match AgilulfClient::connect(address.as_str()).await {
                Err(_) => {}
                Ok(client) => return client,
            }
        }
    }

    async fn multi_connect(server_port: i16, knights_num: usize) -> MultiAgilulfClient {
        let address = format!("127.0.0.1:{}", server_port);
        loop {
            match MultiAgilulfClient::connect(address.as_str(), knights_num).await {
                Err(_) => {}
                Ok(client) => return client,
            }
        }
    }

    fn run_test<F, Fut>(f: F)
    where
        F: FnOnce(i16, ThreadPool) -> Fut + Send + Sync,
        Fut: Future<Output = ()> + Send,
    {
        let mut thread_pool = ThreadPool::builder()
            .name_prefix("test_thread")
            .create()
            .unwrap();

        let port = setup_server();

        let extra_thread_pool = thread_pool.clone();

        thread_pool.run(async move {
            f(port.clone(), extra_thread_pool).await;
        });
    }

    #[test]
    fn put_get_test() {
        run_test(async move |port, _| {
            let client = connect(port).await;
            for i in 0..100 {
                let ans = client
                    .put(
                        Slice(format!("key{}", i).into_bytes()),
                        Slice(format!("value{}", i).into_bytes()),
                    )
                    .await
                    .unwrap();
                assert_eq!(ans, Reply::StatusReply(Status::OK));
            }

            for i in 0..100 {
                let ans = client
                    .get(Slice(format!("key{}", i).into_bytes()))
                    .await
                    .unwrap();
                assert_eq!(
                    ans,
                    Reply::SliceReply(Slice(format!("value{}", i).into_bytes()))
                );
            }
        });
    }

    #[test]
    fn batch_put_test() {
        run_test(async move |port, _| {
            let client = connect(port).await;

            let mut requests = Vec::new();
            for i in 0..100 {
                requests.push(Command::PUT(PutCommand {
                    key: Slice(format!("key{}", i).into_bytes()),
                    value: Slice(format!("value{}", i).into_bytes()),
                }));
            }
            client.send_batch(requests).await.unwrap();

            let mut requests = Vec::new();
            for i in 0..100 {
                requests.push(Command::GET(GetCommand {
                    key: Slice(format!("key{}", i).into_bytes()),
                }));
            }
            let replies = client.send_batch(requests).await.unwrap();

            for i in 0..100 {
                assert_eq!(
                    replies[i],
                    Reply::SliceReply(Slice(format!("value{}", i).into_bytes()))
                );
            }
        });
    }

    #[test]
    fn put_delete_get_test() {
        run_test(async move |port, _| {
            let client = connect(port).await;
            for i in 0..100 {
                let ans = client
                    .put(
                        Slice(format!("key{}", i).into_bytes()),
                        Slice(format!("value{}", i).into_bytes()),
                    )
                    .await
                    .unwrap();
                assert_eq!(ans, Reply::StatusReply(Status::OK));
            }

            for i in 0..100 {
                let ans = client
                    .get(Slice(format!("key{}", i).into_bytes()))
                    .await
                    .unwrap();
                assert_eq!(
                    ans,
                    Reply::SliceReply(Slice(format!("value{}", i).into_bytes()))
                );
            }

            for i in 0..100 {
                if i % 2 == 0 {
                    let ans = client
                        .delete(Slice(format!("key{}", i).into_bytes()))
                        .await
                        .unwrap();
                    assert_eq!(ans, Reply::StatusReply(Status::OK));
                }
            }

            for i in 0..100 {
                let ans = client
                    .get(Slice(format!("key{}", i).into_bytes()))
                    .await
                    .unwrap();
                if i % 2 == 0 {
                    assert_eq!(ans, Reply::ErrorReply(String::from("KeyNotFound\r\n")));
                } else {
                    assert_eq!(
                        ans,
                        Reply::SliceReply(Slice(format!("value{}", i).into_bytes()))
                    );
                }
            }
        });
    }

    #[test]
    fn override_test() {
        run_test(async move |port, _| {
            let client = connect(port).await;
            for i in 0..100 {
                let ans = client
                    .put(
                        Slice(format!("key{}", i).into_bytes()),
                        Slice(format!("value{}", i).into_bytes()),
                    )
                    .await
                    .unwrap();
                assert_eq!(ans, Reply::StatusReply(Status::OK));
            }

            for i in 0..100 {
                let ans = client
                    .get(Slice(format!("key{}", i).into_bytes()))
                    .await
                    .unwrap();
                assert_eq!(
                    ans,
                    Reply::SliceReply(Slice(format!("value{}", i).into_bytes()))
                );
            }

            for i in 0..100 {
                if i % 2 == 0 {
                    let ans = client
                        .put(
                            Slice(format!("key{}", i).into_bytes()),
                            Slice(format!("new_value{}", i).into_bytes()),
                        )
                        .await
                        .unwrap();
                    assert_eq!(ans, Reply::StatusReply(Status::OK));
                }
            }

            for i in 0..100 {
                let ans = client
                    .get(Slice(format!("key{}", i).into_bytes()))
                    .await
                    .unwrap();
                if i % 2 == 0 {
                    assert_eq!(
                        ans,
                        Reply::SliceReply(Slice(format!("new_value{}", i).into_bytes()))
                    );
                } else {
                    assert_eq!(
                        ans,
                        Reply::SliceReply(Slice(format!("value{}", i).into_bytes()))
                    );
                }
            }
        });
    }

    fn generate_keys(num: usize) -> Vec<Vec<u8>> {
        (0..num)
            .map(|_| thread_rng().sample_iter(&Standard).take(8).collect())
            .collect()
    }

    fn generate_values(num: usize) -> Vec<Vec<u8>> {
        (0..num)
            .map(|_| thread_rng().sample_iter(&Standard).take(256).collect())
            .collect()
    }

    fn generate_request(num: usize) -> Vec<Command> {
        let keys: Vec<Vec<u8>> = generate_keys(num);

        let value: Vec<Vec<u8>> = generate_values(num);

        (0..num)
            .map(|index| {
                Command::PUT(PutCommand {
                    key: Slice(keys[index].clone()),
                    value: Slice(value[index].clone()),
                })
            })
            .collect()
    }

    #[test]
    fn batch_put_request() {
        let requests = generate_request(1000);
        let requests = &requests;

        run_test(async move |port, _| {
            let client = connect(port).await;
            let replies = client.send_batch(requests.to_vec()).await.unwrap();
            for reply in replies {
                assert_eq!(reply, Reply::StatusReply(Status::OK))
            }
        });
    }

    #[test]
    fn multi_thread_batch() {
        let requests = generate_request(10000);

        run_test(async move |port, _| {
            let client = multi_connect(port, 128).await;

            client.send_batch(requests.to_vec()).await.unwrap();
        });
    }

    #[bench]
    fn single_thread_bench(b: &mut test::Bencher) {
        let requests = generate_request(1000);

        run_test(async move |port, mut thread_pool| {
            let client = connect(port).await;
            let (sender, receiver) = crossbeam_channel::unbounded::<()>();
            thread_pool
                .spawn(async move {
                    let mut requests = requests.iter().cycle();
                    loop {
                        for _ in 0..1000 {
                            client.send(requests.next().unwrap().clone()).await.unwrap();
                        }
                        match sender.send(()) {
                            Err(_) => {}
                            Ok(_) => {}
                        };
                    }
                })
                .unwrap();

            b.iter(|| {
                receiver.recv().unwrap();
            })
        });
    }

    #[bench]
    fn single_thread_batch_bench(b: &mut test::Bencher) {
        let requests = generate_request(1000);

        run_test(async move |port, mut thread_pool| {
            let client = connect(port).await;
            let (sender, receiver) = crossbeam_channel::unbounded::<()>();

            let client = client.clone();
            let requests = requests.clone();
            let sender = sender.clone();
            thread_pool
                .spawn(async move {
                    loop {
                        client.send_batch(requests.to_vec()).await.unwrap();
                        match sender.send(()) {
                            Err(_) => {}
                            Ok(_) => {}
                        };
                    }
                })
                .unwrap();

            b.iter(|| {
                receiver.recv().unwrap();
            })
        });
    }

    #[bench]
    fn multi_knights_bench(b: &mut test::Bencher) {
        let requests = generate_request(1000);

        run_test(async move |port, mut thread_pool| {
            let client = multi_connect(port, 4).await;
            let (sender, receiver) = crossbeam_channel::unbounded::<()>();
            thread_pool
                .spawn(async move {
                    let mut requests = requests.iter().cycle();
                    loop {
                        for _ in 0..1000 {
                            client.send(requests.next().unwrap().clone()).await.unwrap();
                        }
                        match sender.send(()) {
                            Err(_) => {}
                            Ok(_) => {}
                        };
                    }
                })
                .unwrap();

            b.iter(|| {
                receiver.recv().unwrap();
            })
        });
    }

    #[bench]
    fn multi_thread_batch_bench(b: &mut test::Bencher) {
        let requests = generate_request(1000);

        run_test(async move |port, mut thread_pool| {
            let client = multi_connect(port, 128).await;
            let (sender, receiver) = crossbeam_channel::unbounded::<()>();

            let client = client.clone();
            let requests = requests.clone();
            let sender = sender.clone();
            thread_pool
                .spawn(async move {
                    loop {
                        client.send_batch(requests.to_vec()).await.unwrap();
                        match sender.send(()) {
                            Err(_) => {}
                            Ok(_) => {}
                        };
                    }
                })
                .unwrap();

            b.iter(|| {
                receiver.recv().unwrap();
            })
        });
    }
}
