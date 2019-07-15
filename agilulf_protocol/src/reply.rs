use super::{DatabaseResult, Slice, AsyncReadBuffer, Result, ProtocolError};
use super::message::{MessageHead, PartHead};
use futures::{AsyncWrite, AsyncRead, Stream, Poll, Future, Sink, SinkExt, AsyncWriteExt};
use super::async_buffer::AsyncWriteBuffer;
use futures::task::Context;
use std::pin::Pin;
use crate::Command;
use futures::stream::iter;
use std::collections::VecDeque;
use std::error::Error;

#[derive(PartialEq, Debug)]
pub enum Status {
    OK,
}

#[derive(PartialEq, Debug)]
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

impl From<Vec<(Slice, Slice)>> for Reply {
    fn from(result: Vec<(Slice, Slice)>) -> Self {
        Reply::MultipleSliceReply(result.into_iter().flat_map(|(key, value)| {
            vec![key, value]
        }).collect())
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

async fn read_reply<T: AsyncRead + Unpin>(buf: &mut AsyncReadBuffer<T>) -> Result<Reply> {
    let first_line = buf.read_line().await?;

    if first_line[0] == b'+' {
        Ok(Reply::StatusReply(Status::OK))
    } else if first_line[0] == b'-' {
        Ok(Reply::ErrorReply(std::str::from_utf8(&first_line[1..])?.to_owned()))
    } else if first_line[0] == b'*' {
        let mut slices = Vec::new();

        let head = MessageHead::from_buf(first_line)?;
        for _ in 0..head.count {
            let part = buf.read_line().await?;
            let head = PartHead::from_buf(part)?;
            let mut content = buf.read_exact(head.size + 2).await?; // 2 for \r\n
            let content = content.drain(0..content.len()-2).collect();

            slices.push(Slice(content));
        }

        Ok(Reply::MultipleSliceReply(slices))
    } else if first_line[0] == b'$' {
        let head = PartHead::from_buf(first_line)?;
        let mut content = buf.read_exact(head.size + 2).await?; // 2 for \r\n
        let content = content.drain(0..content.len()-2).collect();

        Ok(Reply::SliceReply(Slice(content)))
    } else {
        Err(ProtocolError::GrammarCheckFailed("Reply Grammar Error"))
    }
}

impl<T: AsyncRead + Unpin + 'static> AsyncReadBuffer<T> {
    pub fn into_reply_stream(self) -> impl Stream<Item = Result<Reply>>  {
        futures::stream::unfold(self,   |mut buffer| {
            let future = async move {
                let command = read_reply(&mut buffer).await;
                Some((command, buffer))
            };
            Box::pin(future)
        })
    }
}

impl<T: AsyncWrite + Unpin + 'static> AsyncWriteBuffer<T> {
    pub fn into_reply_sink(self) -> impl Sink<Reply, Error=ProtocolError> {
        self.stream.into_sink().with(|reply: Reply| {
            let mut reply: Vec<u8> = reply.into();
            futures::future::ready(Result::Ok(reply))
        })
    }
}