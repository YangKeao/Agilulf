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
</p>

## Usage

### Server

Installing and starting a Agilulf Server is quite simple.

```bash
cargo install agilulf

agilulf_server --help
```

You will know what you need.

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

## TODO

- [x] AIO for writing files
- [ ] Wait for `crossbeam-skiplist` to be stable and migrate to it
- [ ] Compact SSTables into higher level
- [ ] Restore data from frozen logs