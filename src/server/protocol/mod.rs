mod error;

pub trait ReadMessage {
    async fn read_message() -> Result;
}
