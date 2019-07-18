#![feature(async_await)]

use agilulf::{DatabaseBuilder, MemDatabase, Server};
use agilulf_driver::MultiAgilulfClient;
use agilulf_protocol::{Command, GetCommand, PutCommand, Reply, Slice};
use futures::executor::ThreadPool;
use rand::distributions::Standard;
use rand::{thread_rng, Rng};
use std::time::{Duration, Instant};

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

fn generate_request(num: usize) -> (Vec<Command>, Vec<Command>) {
    let keys: Vec<Vec<u8>> = generate_keys(num);

    let value: Vec<Vec<u8>> = generate_values(num);

    (
        (0..num)
            .map(|index| {
                Command::PUT(PutCommand {
                    key: Slice(keys[index].clone()),
                    value: Slice(value[index].clone()),
                })
            })
            .collect(),
        (0..num)
            .map(|index| {
                Command::GET(GetCommand {
                    key: Slice(keys[index].clone()),
                })
            })
            .collect(),
    )
}

async fn connect(server_port: i16) -> MultiAgilulfClient {
    let address = format!("127.0.0.1:{}", server_port);
    loop {
        match MultiAgilulfClient::connect(address.as_str(), 128).await {
            Err(_) => {}
            Ok(client) => return client,
        }
    }
}

fn main() {
    let database = DatabaseBuilder::default()
        .restore(false)
        .base_dir("/var/tmp/agilulf".to_string())
        .build()
        .unwrap();

    let server = Server::new("127.0.0.1:7890", database).unwrap();
    std::thread::Builder::new()
        .name(String::from("server_thread"))
        .spawn(|| {
            server.run().unwrap();
        })
        .unwrap();

    let mut thread_pool = ThreadPool::builder()
        .name_prefix("bench_thread")
        .create()
        .unwrap();

    thread_pool.run(async move {
        let client = connect(7890).await;

        let (put_request, get_request) = generate_request(10000);
        let (put_big_requests, get_big_requests) = generate_request(100000);

        let now = Instant::now();
        let mut time = Duration::new(0, 0);
        let mut times = 0;
        loop {
            let start = Instant::now();
            client.send_batch(put_request.clone()).await.unwrap();
            let end = Instant::now();
            time += end.duration_since(start);
            times += 1;
            if end.duration_since(now) > Duration::new(10, 0) {
                time /= times;
                break;
            }
        }
        println!("Send 10000 PUT requests cost: {:?}", time);

        let now = Instant::now();
        let mut time = Duration::new(0, 0);
        let mut times = 0;
        loop {
            let start = Instant::now();
            let get_response = client.send_batch(get_request.clone()).await.unwrap();
            let end = Instant::now();
            time += end.duration_since(start);
            times += 1;
            if end.duration_since(now) > Duration::new(10, 0) {
                time /= times;
                for (index, res) in get_response.iter().enumerate() {
                    match res {
                        Reply::SliceReply(value) => match &put_request[index] {
                            Command::PUT(put) => assert_eq!(value, &put.value),
                            _ => unreachable!(),
                        },
                        _ => unreachable!(),
                    }
                }
                break;
            }
        }
        println!("Send 10000 GET requests cost: {:?}", time);

        let now = Instant::now();
        let mut time = Duration::new(0, 0);
        let mut times = 0;
        loop {
            let start = Instant::now();
            client.send_batch(put_big_requests.clone()).await.unwrap();
            let end = Instant::now();
            time += end.duration_since(start);
            times += 1;
            if end.duration_since(now) > Duration::new(10, 0) {
                time /= times;
                break;
            }
        }
        println!("Send 100000 requests cost: {:?}", time);

        let now = Instant::now();
        let mut time = Duration::new(0, 0);
        let mut times = 0;
        loop {
            let start = Instant::now();
            client
                .put(
                    Slice(generate_keys(1)[0].clone()),
                    Slice(generate_values(1)[0].clone()),
                )
                .await
                .unwrap();
            let end = Instant::now();
            time += end.duration_since(start);
            times += 1;
            if end.duration_since(now) > Duration::new(10, 0) {
                time /= times;
                break;
            }
        }
        println!("Send one PUT cost: {:?}", time);
    });
}
