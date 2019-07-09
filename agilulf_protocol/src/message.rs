use super::{ProtocolError, Result};

pub struct MessageHead {
    pub count: usize,
}

impl MessageHead {
    pub fn from_buf(buf: Vec<u8>) -> Result<MessageHead> {
        if buf[0] == b'*' {
            Ok(MessageHead {
                count: std::str::from_utf8(&buf[1..])?.trim().parse()?,
            })
        } else {
            log::info!("Error buffer is {:?}", std::str::from_utf8(&buf)?);
            Err(ProtocolError::GrammarCheckFailed(
                "* should be the first character of a message",
            ))
        }
    }
    pub fn into_bytes(self) -> Vec<u8> {
        format!("*{}\r\n", self.count).into_bytes()
    }
}

pub struct PartHead {
    pub size: usize,
}

impl PartHead {
    pub fn from_buf(buf: Vec<u8>) -> Result<PartHead> {
        if buf[0] == b'$' {
            Ok(PartHead {
                size: std::str::from_utf8(&buf[1..])?.trim().parse()?,
            })
        } else {
            Err(ProtocolError::GrammarCheckFailed(
                "$ should be the first character of a message part",
            ))
        }
    }
    pub fn into_bytes(self) -> Vec<u8> {
        format!("${}\r\n", self.size).into_bytes()
    }
}
