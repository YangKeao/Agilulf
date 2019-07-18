use memmap::{MmapMut, MmapOptions};
use std::marker::PhantomData;
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

pub trait JudgeReal {
    fn is_real(&self) -> bool;
    fn set_real(&mut self, real: bool);
}

pub struct LogIterator<'a, T> {
    inner_mmap: &'a MmapMut,
    index: usize,
    phantom: PhantomData<T>,
}

impl<'a, T: Clone + JudgeReal> Iterator for LogIterator<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let record =
            self.inner_mmap[(self.index * size_of::<T>())..].as_ref() as *const [u8] as *mut T;
        unsafe {
            if (*record).is_real() {
                self.index += 1;
                return Some((*record).clone());
            } else {
                return None;
            }
        }
    }
}

pub struct LogManager<T: JudgeReal + Clone> {
    inner_mmap: MmapMut,
    index: AtomicUsize,
    phantom: PhantomData<T>,
}

impl<T: JudgeReal + Clone> LogManager<T> {
    pub fn create_new(path: &str, length: usize) -> Result<LogManager<T>> {
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;

        LogManager::open(path, length)
    }

    pub fn open(path: &str, length: usize) -> Result<LogManager<T>> {
        {
            let file = agilulf_fs::File::open(path)?;
            file.fallocate(0, (unsafe { size_of::<T>() } * length) as i64);
        }

        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;

        let mmap = unsafe { MmapOptions::new().map_mut(&file)? };
        let records = mmap.as_ref() as *const [u8] as *mut [T];

        let mut index = 0;
        unsafe {
            let records = &(*records);
            loop {
                if records[index].is_real() {
                    index += 1;
                } else {
                    break;
                }
            }
        }

        Ok(Self {
            inner_mmap: mmap,
            index: AtomicUsize::new(index),
            phantom: PhantomData,
        })
    }

    pub fn iter(&self) -> LogIterator<T> {
        LogIterator {
            inner_mmap: &self.inner_mmap,
            index: 0,
            phantom: PhantomData,
        }
    }

    pub fn add_entry(&self, data: T) {
        let index = self.index.fetch_add(1, Ordering::SeqCst);

        unsafe {
            let record =
                self.inner_mmap[(index * size_of::<T>())..].as_ref() as *const [u8] as *mut T;
            (*record).clone_from(&data);
        }
    }
}
