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

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn message_head() {
        let message_head = MessageHead::from_buf(b"*100\r\n".to_vec()).unwrap();
        assert_eq!(message_head.count, 100);
        assert_eq!(message_head.into_bytes(), b"*100\r\n".to_vec());
    }

    #[test]
    fn part_head() {
        let part_head = PartHead::from_buf(b"$100\r\n".to_vec()).unwrap();
        assert_eq!(part_head.size, 100);
        assert_eq!(part_head.into_bytes(), b"$100\r\n".to_vec());
    }

    #[test]
    fn wrong_message_head() {
        match MessageHead::from_buf(b"$100\r\r".to_vec()) {
            Err(err) => assert_eq!(
                err.description(),
                "* should be the first character of a message"
            ),
            Ok(_) => assert!(false, "should throw an error"),
        }
    }

    #[test]
    fn wrong_part_head() {
        match PartHead::from_buf(b"*100\r\r".to_vec()) {
            Err(err) => assert_eq!(
                err.description(),
                "$ should be the first character of a message part"
            ),
            Ok(_) => assert!(false, "should throw an error"),
        }
    }
}
