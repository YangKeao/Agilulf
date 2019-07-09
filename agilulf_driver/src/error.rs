quick_error! {
    #[derive(Debug)]
    pub enum ClientError {
        AddressError(err: std::net::AddrParseError) {
            from()
        }
        ConnectError(err: std::io::Error) {
            from()
        }
    }
}

pub type Result<T> = std::result::Result<T, ClientError>;
