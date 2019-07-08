pub mod error;
pub mod tcp_buffer;

pub use error::ProtocolError;

use error::Result;
use tcp_buffer::TcpStreamBuffer;

use futures::io::AsyncReadExt;
use futures::Future;

use crate::storage::Slice;
use crate::storage;

use std::error::Error;

pub struct MessageHead {
    pub count: usize,
}

impl MessageHead {
    fn new(buf: Vec<u8>) -> Result<MessageHead> {
        // trim for several \u{0} at start (though I don't know why)
        let mut buf = &buf[..];
        while buf[0] == 0 {
            buf = &buf[1..];
        }

        if buf[0] == '*' as u8 {
            Ok(MessageHead {
                count: std::str::from_utf8(&buf[1..])?.trim().parse()?,
            })
        } else {
            log::info!("Error buffer is {:?}", std::str::from_utf8(&buf)?);
            Err(ProtocolError::GrammarCheckFailed("* should be the first character of a message"))
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
                size: std::str::from_utf8(&buf[1..])?.trim().parse()?,
            })
        } else {
            Err(ProtocolError::GrammarCheckFailed("$ should be the first character of a message part"))
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
        let mut content = buf.read_exact(head.size + 2).await?; // 2 for \r\n
        let content = content.drain(0..content.len()-2).collect();

        message.push(content);
    }

    Ok(message)
}

pub struct PutCommand {
    pub key: Slice,
    pub value: Slice
}

pub struct GetCommand {
    pub key: Slice
}

pub struct ScanCommand {
    pub start: Slice,
    pub end: Slice,
}

pub enum Command {
    PUT(PutCommand),
    GET(GetCommand),
    SCAN(ScanCommand)
}

impl Command {
    fn new(mut message: Vec<Vec<u8>>) -> Result<Command> {
        let command = std::str::from_utf8(&message[0])?.to_uppercase();
        match command.as_ref() {
            "PUT" => {
                if message.len() == 3 {
                    let value = Slice(message.remove(2));
                    let key = Slice(message.remove(1));
                    Ok(Command::PUT(PutCommand {
                        key,
                        value,
                    }))
                } else {
                    Err(ProtocolError::GrammarCheckFailed("PUT should have two arguments"))
                }
            }
            "GET" => {
                if message.len() == 2 {
                    let key = Slice(message.remove(1));
                    Ok(Command::GET(GetCommand {
                        key,
                    }))
                } else {
                    Err(ProtocolError::GrammarCheckFailed("GET should have one argument"))
                }
            }
            "SCAN" => {
                if message.len() == 3 {
                    let end = Slice(message.remove(2));
                    let start = Slice(message.remove(1));
                    Ok(Command::SCAN(ScanCommand {
                        start,
                        end,
                    }))
                } else {
                    Err(ProtocolError::GrammarCheckFailed("SCAN should have two arguments"))
                }
            }
            _ => Err(ProtocolError::CommandNotSupport(command))
        }
    }
}

pub async fn read_command(buf: &mut TcpStreamBuffer) -> Result<Command> {
    let mut message = read_message(buf).await?;

    Ok(Command::new(message)?)
}

pub enum StatusReply {
    OK,
}

pub enum Reply {
    StatusReply(StatusReply),
    ErrorReply(String),
    SliceReply(Slice),
    MultipleSliceReply(Vec<Slice>),
}

impl From<storage::error::Result<()>> for Reply {
    fn from(result: storage::error::Result<()>) -> Self {
        match result {
            Ok(_) => Reply::StatusReply(StatusReply::OK),
            Err(err) => Reply::ErrorReply(err.description().to_string())
        }
    }
}

impl From<storage::error::Result<Slice>> for Reply {
    fn from(result: storage::error::Result<Slice>) -> Self {
        match result {
            Ok(slice) => Reply::SliceReply(slice),
            Err(err) => Reply::ErrorReply(err.description().to_string())
        }
    }
}

impl From<storage::error::Result<Vec<Slice>>> for Reply {
    fn from(result: storage::error::Result<Vec<Slice>>) -> Self {
        match result {
            Ok(slices) => Reply::MultipleSliceReply(slices),
            Err(err) => Reply::ErrorReply(err.description().to_string())
        }
    }
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
