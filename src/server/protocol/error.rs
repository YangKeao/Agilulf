quick_error! {
    #[derive(Debug)]
    pub enum ProtocolError {

    }
}

pub type Result<T> = std::result::Result<T, ProtocolError>;
