quick_error! {
    #[derive(Debug)]
    pub enum ProtocolError {
        GrammarCheckFailed(s: &'static str) {
            description(s)
        }
        ConnectionClosed
        CommandNotSupport(s: String) {
            display("Command {} is not supported", s)
        }
        Utf8Error(err: std::str::Utf8Error) {
            from()
        }
        ParseError(err: std::num::ParseIntError) {
            from()
        }
        IOError(err: std::io::Error) {
            from()
        }
    }
}

pub type Result<T> = std::result::Result<T, ProtocolError>;
