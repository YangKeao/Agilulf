use super::sstable::SSTableError;
use crate::log::LogError;
use agilulf_protocol::error::database_error::DatabaseError;

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
        BackgroundWorkerChannelSendError(err: futures::channel::mpsc::TrySendError<usize>) {
            from()
        }
        RestoreError(err:DatabaseError ) {
            from()
        }
    }
}
pub type StorageResult<T> = std::result::Result<T, StorageError>;
