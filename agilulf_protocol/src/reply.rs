use super::{DatabaseResult, Slice, TcpStreamBuffer, Result};
use std::error::Error;

pub enum Status {
    OK,
}

pub enum Reply {
    StatusReply(Status),
    ErrorReply(String),
    SliceReply(Slice),
    MultipleSliceReply(Vec<Slice>),
}

impl From<DatabaseResult<()>> for Reply {
    fn from(result: DatabaseResult<()>) -> Self {
        match result {
            Ok(_) => Reply::StatusReply(Status::OK),
            Err(err) => Reply::ErrorReply(err.description().to_string()),
        }
    }
}

impl From<DatabaseResult<Slice>> for Reply {
    fn from(result: DatabaseResult<Slice>) -> Self {
        match result {
            Ok(slice) => Reply::SliceReply(slice),
            Err(err) => Reply::ErrorReply(err.description().to_string()),
        }
    }
}

impl From<DatabaseResult<Vec<Slice>>> for Reply {
    fn from(result: DatabaseResult<Vec<Slice>>) -> Self {
        match result {
            Ok(slices) => Reply::MultipleSliceReply(slices),
            Err(err) => Reply::ErrorReply(err.description().to_string()),
        }
    }
}

impl Into<Vec<u8>> for Reply {
    fn into(self) -> Vec<u8> {
        let mut reply: Vec<u8> = Vec::new();
        match self {
            Reply::StatusReply(status) => match status {
                Status::OK => {
                    reply.extend_from_slice(b"+OK\r\n");
                }
            },
            Reply::ErrorReply(err) => {
                reply.extend_from_slice(format!("-{}\r\n", err).as_bytes());
            }
            Reply::SliceReply(slice) => {
                reply.extend_from_slice(format!("${}\r\n", slice.0.len()).as_bytes());
                reply.extend_from_slice(slice.0.as_slice());
                reply.extend_from_slice(b"\r\n");
            }
            Reply::MultipleSliceReply(slices) => {
                reply.extend_from_slice(format!("*{}\r\n", slices.len()).as_bytes());
                for slice in slices {
                    reply.extend_from_slice(format!("${}\r\n", slice.0.len()).as_bytes());
                    reply.extend_from_slice(slice.0.as_slice());
                    reply.extend_from_slice(b"\r\n");
                }
            }
        }
        reply
    }
}

pub async fn send_reply(stream: &mut TcpStreamBuffer, reply: Reply) -> Result<()> {
    let reply = reply.into();
    stream.write_all(reply).await?;
    Ok(())
}