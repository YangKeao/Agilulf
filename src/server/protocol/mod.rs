pub mod error;
pub mod tcp_buffer;

pub use error::ProtocolError;
use error::Result;

use futures::io::AsyncReadExt;

pub trait ReadMessage {
    async fn read_message(&mut self) -> Result<Message>;
}

impl ReadMessage for TcpStream {
    fn read_message(&mut self) -> Result<Vec<&[u8]>> {}
}
