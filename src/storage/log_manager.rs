use super::sstable::PART_LENGTH;
use super::Result as DatabaseResult;
use super::SyncDatabase;
use agilulf_protocol::error::database_error::DatabaseError;
use agilulf_protocol::{Command, DeleteCommand, PutCommand, Slice};
use memmap::{MmapMut, MmapOptions};
use std::mem::size_of;
use std::sync::atomic::{AtomicUsize, Ordering};

quick_error! {
    #[derive(Debug)]
    pub enum LogError {
        FSError(err: agilulf_fs::error::FSError) {
            from()
        }
        IOError(err: std::io::Error) {
            from()
        }
    }
}
pub type Result<T> = std::result::Result<T, LogError>;

#[repr(packed)]
struct RawRecord {
    pub real_flag: u8,
    pub key: [u8; 8],
    pub delete_flag: u8,
    pub value: [u8; 256],
}

pub struct LogIterator<'a> {
    inner_mmap: &'a MmapMut,
    index: usize,
}

impl<'a> Iterator for LogIterator<'a> {
    type Item = Command;

    fn next(&mut self) -> Option<Self::Item> {
        let record = self.inner_mmap[(self.index * size_of::<RawRecord>())..].as_ref()
            as *const [u8] as *mut RawRecord;
        unsafe {
            if (*record).real_flag == 0 {
                return None;
            } else {
                let key = Slice((*record).key.to_vec());
                let delete_flag = (*record).delete_flag;

                let command = match delete_flag {
                    0 => {
                        let value = Slice((*record).value.to_vec());
                        Command::PUT(PutCommand { key, value })
                    }
                    1 => Command::DELETE(DeleteCommand { key }),
                    _ => unreachable!(),
                };
                self.index += 1;
                return Some(command);
            }
        }
    }
}

pub struct LogManager {
    inner_mmap: MmapMut,
    index: AtomicUsize,
}

impl LogManager {
    pub fn create_new(path: &str) -> Result<LogManager> {
        std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(path)?;

        LogManager::open(path)
    }
    pub fn open(path: &str) -> Result<LogManager> {
        {
            let file = agilulf_fs::File::open(path)?;
            file.fallocate(0, ((PART_LENGTH + 2) * 4 * 1024 * 2) as i64);
        }

        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;

        let mmap = unsafe { MmapOptions::new().map_mut(&file)? };
        let records = mmap.as_ref() as *const [u8] as *mut [RawRecord];

        let mut index = 0;
        unsafe {
            let records = &(*records);
            loop {
                if records[index].real_flag == 1 {
                    index += 1;
                } else {
                    break;
                }
            }
        }

        Ok(LogManager {
            inner_mmap: mmap,
            index: AtomicUsize::new(index),
        })
    }

    pub fn export(&self) -> Vec<(Slice, Slice)> {
        let mut vec = Vec::new();

        let records = self.inner_mmap.as_ref() as *const [u8] as *mut [RawRecord];
        unsafe {
            let records = &(*records);

            for index in 0..(self.index.load(Ordering::SeqCst) - 1) {
                let key =
                    Vec::from_raw_parts(&records[index].key as *const [u8; 8] as *mut u8, 8, 8);
                let value = Vec::from_raw_parts(
                    &records[index].value as *const [u8; 256] as *mut u8,
                    256,
                    256,
                );
                vec.push((Slice(key).clone(), Slice(value).clone()));
            }
        }
        vec
    }

    pub fn iter(&self) -> LogIterator {
        LogIterator {
            inner_mmap: &self.inner_mmap,
            index: 0,
        }
    }
}

impl SyncDatabase for LogManager {
    fn get(&self, key: Slice) -> DatabaseResult<Slice> {
        panic!("Cannot read from log directly")
    }

    fn put(&self, key: Slice, value: Slice) -> DatabaseResult<()> {
        let index = self.index.fetch_add(1, Ordering::SeqCst);

        unsafe {
            let record = self.inner_mmap[(index * size_of::<RawRecord>())..].as_ref()
                as *const [u8] as *mut RawRecord;
            (*record).real_flag = 1;
            (*record).key[0..key.0.len()].clone_from_slice(key.0.as_ref());
            (*record).delete_flag = 0;
            (*record).value[0..value.0.len()].clone_from_slice(value.0.as_ref());
        }
        Ok(())
    }

    fn scan(&self, start: Slice, end: Slice) -> Vec<(Slice, Slice)> {
        panic!("Cannot read from log directly")
    }

    fn delete(&self, key: Slice) -> DatabaseResult<()> {
        let index = self.index.fetch_add(1, Ordering::SeqCst);

        unsafe {
            let record = self.inner_mmap[(index * size_of::<RawRecord>())..].as_ref()
                as *const [u8] as *mut RawRecord;
            (*record).real_flag = 1;
            (*record).key.clone_from_slice(key.0.as_ref());
            (*record).delete_flag = 1;
        }
        Ok(())
    }
}
