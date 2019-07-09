quick_error! {
    #[derive(Debug)]
    pub enum ClientError {
        AddressError(err: std::net::AddrParseError) {
            from()
        }
        ConnectError(err: std::io::Error) {
            from()
        }
        ProtocolError(err: agilulf_protocol::error::protocol_error::ProtocolError) {
            from()
        }
    }
}

pub type Result<T> = std::result::Result<T, ClientError>;
