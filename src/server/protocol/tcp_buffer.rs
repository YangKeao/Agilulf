use super::error::Result;
use futures::io::AsyncReadExt;
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
            buffer: Vec::with_capacity(DEFAULT_BUF_SIZE),

            cap: DEFAULT_BUF_SIZE,
            pos: 0,
        }
    }

    pub async fn fill_buf(&mut self) -> Result<&[u8]> {
        if self.pos >= self.cap {
            debug_assert!(self.pos == self.cap);
            self.cap = self.stream.read(&mut self.buf).await;
            self.pos = 0;
        }
        Ok(&self.buf[self.pos..self.cap])
    }

    pub fn consume(&mut self, amt: usize) {
        self.pos = std::cmp::min(self.pos + amt, self.cap);
    }

    pub async fn read_until(&mut self, delim: (u8, u8), buf: &mut Vec<u8>) -> Result<usize> {
        let mut read = 0;
        loop {
            let (done, used) = {
                let available = self.fill_buf().await?;

                match memchr::memchr2(delim.0, delim.1, available) {
                    Some(i) => {
                        buf.extend_from_slice(&available[..=i]);
                        (true, i + 1)
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
                return Ok(read);
            }
        }
    }

    pub async fn read_line(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        self.read_until(('\r' as u8, '\n' as u8), buf)
    }
}
