quick_error! {
    #[derive(Debug)]
    pub enum DatabaseError {
        KeyNotFound
        InternalError(err: String)
    }
}
pub type Result<T> = std::result::Result<T, DatabaseError>;
