quick_error! {
    #[derive(Debug)]
    pub enum ProtocolError {
        GrammarCheckFailed(s: &'static str) {
            description(s)
        }
        Utf8Error(err: std::str::Utf8Error) {
            from()
        }
        ParseError(err: std::num::ParseIntError) {
            from()
        }
    }
}

pub type Result<T> = std::result::Result<T, ProtocolError>;
