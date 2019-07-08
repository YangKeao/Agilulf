pub mod error;
pub mod tcp_buffer;

pub use error::ProtocolError;

use error::Result;
use tcp_buffer::TcpStreamBuffer;

use futures::io::AsyncReadExt;
use futures::Future;

pub struct MessageHead {
    pub count: usize,
}

impl MessageHead {
    fn new(buf: Vec<u8>) -> Result<MessageHead> {
        if buf[0] == '*' as u8 {
            Ok(MessageHead {
                count: std::str::from_utf8(&buf[1..])?.parse()?,
            })
        } else {
            Err(ProtocolError::GrammarCheckFailed("$ should be the first character of a message"))
        }
    }
}

pub struct PartHead {
    pub size: usize,
}

impl PartHead {
    fn new(buf: Vec<u8>) -> Result<PartHead> {
        if buf[0] == '$' as u8 {
            Ok(PartHead {
                size: std::str::from_utf8(&buf[1..])?.parse()?,
            })
        } else {
            Err(ProtocolError::GrammarCheckFailed("$ should be the first character of a message"))
        }
    }
}

pub async fn read_message(buf: &mut TcpStreamBuffer) -> Result<Vec<Vec<u8>>> {
    let mut message = Vec::new();

    let mut line = buf.read_line().await?;
    let head = MessageHead::new(line)?;

    for _ in 0..head.count {
        let part = buf.read_line().await?;
        let head = PartHead::new(part)?;
        let content = buf.read_exact(head.size).await?;

        message.push(content);
    }

    Ok(message)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_head() {
        let buf = b"*12".to_vec();
        let head = MessageHead::new(buf).unwrap();
        assert_eq!(head.count, 12);
    }
}
