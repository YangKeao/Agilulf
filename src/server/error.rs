quick_error! {
    #[derive(Debug)]
    pub enum ServerError {
        AddressParseError(err: std::net::AddrParseError) {
            from()
        }
        IOError(err: std::io::Error) {
            from()
        }
        SpawnError(err: futures::task::SpawnError) {
            from()
        }
    }
}

pub type Result<T> = std::result::Result<T, ServerError>;
