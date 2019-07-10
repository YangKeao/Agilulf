use super::{ProtocolError, Result};
use futures::io::{AsyncReadExt, AsyncWriteExt};
use futures::{AsyncRead, AsyncWrite};

pub const DEFAULT_BUF_SIZE: usize = 8 * 1024;

pub struct AsyncReadBuffer<T: AsyncRead + Unpin> {
    stream: T,
    read_buffer: Vec<u8>,

    read_pos: usize,
    read_cap: usize,
}

impl<T: AsyncRead + Unpin> AsyncReadBuffer<T> {
    pub fn new(stream: T) -> AsyncReadBuffer<T> {
        AsyncReadBuffer {
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

    pub async fn read_line(&mut self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        loop {
            let (done, used) = {
                let available = self.fill_buf().await?;

                let index = memchr::memchr(b'\n', available);
                if index.is_some() &&
                    ((index.unwrap() > 0 && available[index.unwrap() - 1] == b'\r')
                        || (index.unwrap() == 0 && !buf.is_empty() && buf[buf.len() - 1] == b'\r')) {
                    let index = index.unwrap();
                    buf.extend_from_slice(&available[..=index]);

                    (true, index + 1)
                } else {
                    buf.extend_from_slice(available);
                    (false, available.len())
                }
            };
            self.consume(used);

            if done || used == 0 {
                return Ok(buf);
            }
        }
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
}

pub struct AsyncWriteBuffer<T: AsyncWrite + Unpin> {
    stream: T,
}

impl<T: AsyncWrite + Unpin> AsyncWriteBuffer<T> {
    pub fn new(stream: T) -> AsyncWriteBuffer<T> {
        AsyncWriteBuffer {
            stream,
        }
    }

    pub async fn write_all(&mut self, data: Vec<u8>) -> Result<()> {
        Ok(self.stream.write_all(data.as_slice()).await?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Once;
    use futures::executor::{self, ThreadPool};
    use romio::{TcpStream, TcpListener};
    use futures::task::SpawnExt;
    use std::net::SocketAddr;
    use futures::{StreamExt, AsyncWriteExt};
    use crate::AsyncReadBuffer;

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
            let mut buffer = AsyncReadBuffer::new(stream);

            let line = buffer.read_line().await.unwrap();
            let line = std::str::from_utf8(line.as_slice()).unwrap();
            assert_eq!(line, "TEST LINE 1\r\n");
        };

        futures::executor::block_on(future);
    }

    #[test]
    fn read_exact() {
        let future = async {
            let stream = start_server().await;
            let mut buffer = AsyncReadBuffer::new(stream);

            let exact = buffer.read_exact(8).await.unwrap();
            let exact = std::str::from_utf8(exact.as_slice()).unwrap();
            assert_eq!(exact, "TEST LIN");

            buffer.read_line().await.unwrap();

            let exact = buffer.read_exact(8).await.unwrap();
            let exact = std::str::from_utf8(exact.as_slice()).unwrap();
            assert_eq!(exact, "TESTTEST");
        };

        futures::executor::block_on(future);
    }
}
