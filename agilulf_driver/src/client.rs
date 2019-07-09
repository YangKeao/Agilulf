use std::net::SocketAddr;

use agilulf_protocol::Reply;
use romio::TcpStream;

use super::error::{ClientError, Result};

pub struct AgilulfClient {
    stream: TcpStream,
}

impl AgilulfClient {
    async fn new(address: &str) -> Result<AgilulfClient> {
        let addr = address.parse::<SocketAddr>()?;
        let stream = TcpStream::connect(&addr).await?;

        Ok(AgilulfClient { stream })
    }

    async fn put(&self) -> Result<Reply> {}
}
