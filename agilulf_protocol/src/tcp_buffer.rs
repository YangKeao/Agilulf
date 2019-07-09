use super::{ProtocolError, Result};
use futures::io::{AsyncReadExt, AsyncWriteExt};
use romio::TcpStream;

pub const DEFAULT_BUF_SIZE: usize = 8 * 1024;

pub struct TcpStreamBuffer {
    stream: TcpStream,
    read_buffer: Vec<u8>,

    read_pos: usize,
    read_cap: usize,
}

impl TcpStreamBuffer {
    pub fn new(stream: TcpStream) -> TcpStreamBuffer {
        TcpStreamBuffer {
            stream,
            read_buffer: vec![0; DEFAULT_BUF_SIZE],

            read_cap: 0,
            read_pos: 0,
        }
    }

    pub async fn fill_buf(&mut self) -> Result<&[u8]> {
        if self.read_pos >= self.read_cap {
            debug_assert_eq!(self.read_pos, self.read_cap);
            self.read_cap = self.stream.read(&mut self.read_buffer).await?;
            if self.read_cap == 0 {
                return Err(ProtocolError::ConnectionClosed);
            }
            self.read_pos = 0;
        }
        Ok(&self.read_buffer[self.read_pos..self.read_cap])
    }

    pub fn consume(&mut self, amt: usize) {
        self.read_pos = std::cmp::min(self.read_pos + amt, self.read_cap);
    }

    pub async fn read_until(&mut self, delim: (u8, u8)) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        loop {
            let (done, used) = {
                let available = self.fill_buf().await?;

                match memchr::memchr2(delim.0, delim.1, available) {
                    Some(i) => {
                        buf.extend_from_slice(&available[..i + 2]);
                        (true, i + 2)
                    }
                    None => {
                        buf.extend_from_slice(available);
                        (false, available.len())
                    }
                }
            };
            self.consume(used);

            if done || used == 0 {
                return Ok(buf);
            }
        }
    }

    pub async fn read_line(&mut self) -> Result<Vec<u8>> {
        self.read_until((b'\r', b'\n')).await
    }

    pub async fn read_exact(&mut self, size: usize) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        let mut read = 0;
        loop {
            let (done, used) = {
                let available = self.fill_buf().await?;

                if read + available.len() >= size {
                    buf.extend_from_slice(&available[..size - read]);
                    (true, size - read)
                } else {
                    buf.extend_from_slice(available);
                    (false, available.len())
                }
            };
            self.consume(used);
            read += used;

            if done || used == 0 {
                return Ok(buf);
            }
        }
    }

    pub async fn write_all(&mut self, buf: Vec<u8>) -> Result<()> {
        self.stream.write_all(buf.as_slice()).await?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::sync::Once;
    use futures::executor::{self, ThreadPool};
    use romio::{TcpStream, TcpListener};
    use futures::task::SpawnExt;
    use std::net::SocketAddr;
    use futures::{StreamExt, AsyncWriteExt};
    use crate::TcpStreamBuffer;

    const ADDRESS: &str = "127.0.0.1:7999";
    static START_SERVER: Once = Once::new();

    async fn start_server() -> TcpStream {
        START_SERVER.call_once(|| {
            std::thread::spawn(|| {
                executor::block_on(async {
                    let mut thread_pool = ThreadPool::new().unwrap();

                    let addr = ADDRESS.parse::<SocketAddr>().unwrap();
                    let mut listener = TcpListener::bind(&addr).unwrap();

                    let mut incoming = listener.incoming();

                    while let Some(stream) = incoming.next().await {
                        let mut stream: TcpStream = stream.unwrap();

                        thread_pool.spawn(async move {
                            stream.write_all(b"TEST LINE 1\r\nTESTTESTTEST\r\n").await.unwrap();
                            std::mem::forget(stream);
                        }).unwrap();
                    }
                });
            });
        });
        let addr = ADDRESS.parse::<SocketAddr>().unwrap();
        loop {
            match TcpStream::connect(&addr).await {
                Err(_) => {},
                Ok(stream) => return stream
            }
        }
    }

    #[test]
    fn read_line() {
        let future = async {
            let stream = start_server().await;
            let mut buffer = TcpStreamBuffer::new(stream);

            let mut line = buffer.read_line().await.unwrap();

            let line = std::str::from_utf8(line.as_slice()).unwrap();
            assert_eq!(line, "TEST LINE 1\r\n");
        };

        futures::executor::block_on(future);
    }
}
