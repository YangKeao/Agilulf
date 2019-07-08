use super::error::Result;
use crate::server::protocol::ProtocolError;
use futures::io::AsyncReadExt;
use futures::Future;
use romio::TcpStream;

pub const DEFAULT_BUF_SIZE: usize = 8 * 1024;

pub struct TcpStreamBuffer {
    stream: TcpStream,
    buffer: Vec<u8>,

    pos: usize,
    cap: usize,
}

impl TcpStreamBuffer {
    pub fn new(stream: TcpStream) -> TcpStreamBuffer {
        TcpStreamBuffer {
            stream,
            buffer: vec![0; DEFAULT_BUF_SIZE],

            cap: DEFAULT_BUF_SIZE,
            pos: 0,
        }
    }

    pub async fn fill_buf(&mut self) -> Result<&[u8]> {
        if self.pos >= self.cap {
            debug_assert_eq!(self.pos, self.cap);
            self.cap = self.stream.read(&mut self.buffer).await?;
            if self.cap == 0 {
                return Err(ProtocolError::ConnectionClosed);
            }
            self.pos = 0;
        }
        Ok(&self.buffer[self.pos..self.cap])
    }

    pub fn consume(&mut self, amt: usize) {
        self.pos = std::cmp::min(self.pos + amt, self.cap);
    }

    pub async fn read_until(&mut self, delim: (u8, u8)) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        let mut read = 0;
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
            read += used;

            if done || used == 0 {
                return Ok(buf);
            }
        }
    }

    pub async fn read_line(&mut self) -> Result<Vec<u8>> {
        self.read_until(('\r' as u8, '\n' as u8)).await
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
