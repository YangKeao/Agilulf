quick_error! {
    #[derive(Debug)]
    pub enum DatabaseError {
        KeyNotFound
    }
}
pub type Result<T> = std::result::Result<T, DatabaseError>;
