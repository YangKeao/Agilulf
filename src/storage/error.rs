use super::sstable::SSTableError;
use crate::log::LogError;

quick_error! {
    #[derive(Debug)]
    pub enum StorageError {
        UnicodeError
        ManifestLogFormatError
        IOError(err: std::io::Error) {
            from()
        }
        LogManagerError(err: LogError) {
            from()
        }
        SSTableError(err: SSTableError) {
            from()
        }
    }
}
pub type StorageResult<T> = std::result::Result<T, StorageError>;
