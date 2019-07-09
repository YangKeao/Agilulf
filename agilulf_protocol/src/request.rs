use super::{Slice, ProtocolError, TcpStreamBuffer,Result };

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
    fn into_bytes(self) -> Vec<u8> {
        format!("*{}\r\n", self.count).into_bytes()
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
    fn into_bytes(self) -> Vec<u8> {
        format!("*{}\r\n", self.size).into_bytes()
    }
}

pub async fn read_message(buf: &mut TcpStreamBuffer) -> Result<Vec<Vec<u8>>> {
    let mut message = Vec::new();

    let line = buf.read_line().await?;
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

pub struct DeleteCommand {
    pub key: Slice,
}

pub enum Command {
    PUT(PutCommand),
    GET(GetCommand),
    DELETE(DeleteCommand),
    SCAN(ScanCommand)
}

impl Command {
    fn from_message(mut message: Vec<Vec<u8>>) -> Result<Command> {
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
            "DELETE" => {
                if message.len() == 2 {
                    let key = Slice(message.remove(1));
                    Ok(Command::DELETE(DeleteCommand {
                        key,
                    }))
                } else {
                    Err(ProtocolError::GrammarCheckFailed("DELETE should have one argument"))
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
    let message = read_message(buf).await?;

    Ok(Command::from_message(message)?)
}

trait AppendSlice {
    fn append_part(&mut self, slice: &[u8]);
}

impl AppendSlice for Vec<u8> {
    fn append_part(&mut self, slice: &[u8]) {
        self.extend_from_slice((PartHead {
            size: slice.len()
        }).into_bytes().as_slice());
        self.extend_from_slice(slice);
        self.extend_from_slice(b"\r\n");
    }
}

impl Into<Vec<u8>> for Command {
    fn into(self) -> Vec<u8> {
        let mut message = Vec::new();
        match self {
            Command::PUT(command) => {
                message.extend_from_slice((MessageHead {
                    count: 3
                }).into_bytes().as_slice());

                message.append_part(b"PUT");
                message.append_part(command.key.0.as_slice());
                message.append_part(command.value.0.as_slice());
            }
            Command::GET(command) => {
                message.extend_from_slice((MessageHead {
                    count: 2
                }).into_bytes().as_slice());

                message.append_part(b"PUT");
                message.append_part(command.key.0.as_slice());
            }
            Command::DELETE(command) => {
                message.extend_from_slice((MessageHead {
                    count: 2
                }).into_bytes().as_slice());

                message.append_part(b"DELETE");
                message.append_part(command.key.0.as_slice());
            }
            Command::SCAN(command) => {
                message.extend_from_slice((MessageHead {
                    count: 3
                }).into_bytes().as_slice());

                message.append_part(b"SCAN");
                message.append_part(command.start.0.as_slice());
                message.append_part(command.end.0.as_slice());
            }
        }

        message
    }
}