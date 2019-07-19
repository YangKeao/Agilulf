<p align="center">
    <img src="https://upload.wikimedia.org/wikipedia/commons/4/42/Nuremberg_chronicles_f_150r_3.jpg" alt="Agilulf">
</p>

<p align="center">
    A fast and asynchronous KV database
</p>

<p align="center">
  <a href="https://travis-ci.com/YangKeao/Agilulf">
    <img alt="Build Status" src="https://travis-ci.com/YangKeao/Agilulf.svg?branch=master">
  </a>

  <a href="https://crates.io/crates/agilulf">
    <img alt="Crates.io" src="https://img.shields.io/crates/v/agilulf.svg">
  </a>
  
  <a href="https://yangkeao.github.io/Agilulf/agilulf/">
    <img alt="Docs" src="https://img.shields.io/badge/docs-current-success.svg">
  </a>
</p>

## Usage

Detail about the protocol can be found in [documents about protocol](https://github.com/YangKeao/Agilulf/tree/master/agilulf_protocol).

### Server

Installing and starting a Agilulf Server is quite simple.

```bash
cargo install agilulf

agilulf_server --help
```

If you want to start a memory only server (like redis without persistence)

```bash
agilulf_server --mem --addr <ADDR>
```

If you need a server with data persistence on disk (with sstable and LSM tree structure. However, compaction
is not implemented yet. So it is much slower after responding to much PUT requests)

```bash
agilulf_server --addr <ADDR>
```

### Client

If you need a client, [agilulf_driver](https://github.com/YangKeao/Agilulf/tree/master/agilulf_driver)
is an asynchronous Rust client for this protocol.

Add this to your Cargo.toml:

```toml
[dependencies]
agilulf_driver = "0.1.0"
```

Now you can use the client:

```rust
use agilulf_driver::MultiAgilulfClient;

futures::executor::block_on(async {
    let client = MultiAgilulfClient::connect(address, 128).await;
    
    client.put(Slice(b"HELLO".to_vec()), Slice(b"WORLD".to_vec())).await;
})
```

### Benchmark

A simple benchmark is provided in [agilulf_driver/examples](https://github.com/YangKeao/Agilulf/tree/master/agilulf_driver/examples)
the `benchmark.rs` example will start up a server itself. And then send request to that server.

```bash
cargo run --example benchmark --release
```

**Note:** It will use `/var/tmp/agilulf` as it's base directory, so make sure this directory exists. Agilulf will
not create directory for you.

Another choice is `remote_benchmark.rs`

```bash
cargo run --example remote_benchmark --release -- --addr <ADDR>
```

You can setup a server yourself and send requests to that server. The default addr is "127.0.0.1:3421" which
is same with default addr of Agilulf Server.

## TODO

- [x] AIO for writing files
- [ ] Wait for `crossbeam-skiplist` to be stable and migrate to it
- [ ] Compact SSTables into higher level
- [ ] Restore data from frozen logs