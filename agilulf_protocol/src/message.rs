use super::{ProtocolError, Result};

pub struct MessageHead {
    pub count: usize,
}

impl MessageHead {
    pub fn from_buf(buf: Vec<u8>) -> Result<MessageHead> {
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
            Err(ProtocolError::GrammarCheckFailed(
                "* should be the first character of a message",
            ))
        }
    }
    fn into_bytes(self) -> Vec<u8> {
        format!("*{}\r\n", self.count).into_bytes()
    }
}

pub struct PartHead {
    pub size: usize,
}

impl PartHead {
    pub fn from_buf(buf: Vec<u8>) -> Result<PartHead> {
        if buf[0] == '$' as u8 {
            Ok(PartHead {
                size: std::str::from_utf8(&buf[1..])?.trim().parse()?,
            })
        } else {
            Err(ProtocolError::GrammarCheckFailed(
                "$ should be the first character of a message part",
            ))
        }
    }
    fn into_bytes(self) -> Vec<u8> {
        format!("*{}\r\n", self.size).into_bytes()
    }
}
