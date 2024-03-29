use super::{mem_database::MemDatabase, SyncDatabase};
use agilulf_protocol::Slice;
use agilulf_protocol::{DatabaseError, DatabaseResult as Result};
use memmap::MmapOptions;
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::ops::{Index, Range};

pub trait SearchIndex:
    Index<usize, Output = (Slice, Slice)>
    + Index<std::ops::Range<usize>, Output = [(Slice, Slice)]>
    + Sync
    + Send
{
    fn len(&self) -> usize;

    fn binary_search_by_key(&self, key: &Slice) -> usize {
        let mut size = self.len();
        if size == 0 {
            panic!("Shouldn't `binary_search` on an empty SearchIndex")
        }
        let mut base = 0usize;
        while size > 1 {
            let half = size / 2;
            let mid = base + half;

            let cmp = self[mid].0.cmp(key);
            base = if cmp == Ordering::Greater { base } else { mid };
            size -= half;
        }

        base
    }

    fn first(&self) -> &(Slice, Slice) {
        &self[0]
    }

    fn last(&self) -> &(Slice, Slice) {
        &self[self.len() - 1]
    }
}

#[repr(packed)]
pub struct SSTable {
    kv_pairs: Box<dyn SearchIndex>,
}

impl SyncDatabase for SSTable {
    fn get_sync(&self, key: Slice) -> Result<Slice> {
        if &self.kv_pairs.first().0 > &key || &self.kv_pairs.last().0 < &key {
            return Err(DatabaseError::KeyNotFound);
        }
        let index = self.kv_pairs.binary_search_by_key(&key);

        if self.kv_pairs[index].0.cmp(&key) == Ordering::Equal {
            Ok(self.kv_pairs[index].1.clone())
        } else {
            Err(DatabaseError::KeyNotFound)
        }
    }

    fn put_sync(&self, _: Slice, _: Slice) -> Result<()> {
        panic!("Cannot modify SSTable")
    }

    fn scan_sync(&self, start: Slice, end: Slice) -> Vec<(Slice, Slice)> {
        let start_index = self.kv_pairs.binary_search_by_key(&start);
        let end_index = self.kv_pairs.binary_search_by_key(&end);
        self.kv_pairs[start_index..end_index].to_vec()
    }

    fn delete_sync(&self, _: Slice) -> Result<()> {
        panic!("Cannot modify SSTable")
    }
}

impl SearchIndex for Vec<(Slice, Slice)> {
    fn len(&self) -> usize {
        Vec::len(self)
    }
}

impl<T: Borrow<MemDatabase>> From<T> for SSTable {
    fn from(mem_database: T) -> Self {
        let kv_pairs: Box<Vec<(Slice, Slice)>> = box mem_database
            .borrow()
            .scan_sync(Slice(Vec::new()), Slice(vec![255; 8]))
            .into_iter()
            .collect();

        Self { kv_pairs }
    }
}

pub const PART_LENGTH: usize = 256 + 8;

struct SliceMmap {
    _inner_mmap: memmap::Mmap,
    inner_vec: Option<Vec<(Slice, Slice)>>,
}

impl Drop for SliceMmap {
    fn drop(&mut self) {
        let inner_vec = self.inner_vec.take();
        std::mem::forget(inner_vec);
    }
}

impl SliceMmap {
    fn from_mmap(mmap: memmap::Mmap) -> Self {
        unsafe {
            let length = mmap.len() / PART_LENGTH;
            let mut inner_vec = Vec::new();

            for index in 0..length {
                let key = Vec::from_raw_parts(
                    mmap.index(PART_LENGTH * index) as *const u8 as *mut u8, // Note: don't write to this vector
                    8,
                    8,
                );
                let value = Vec::from_raw_parts(
                    mmap.index(PART_LENGTH * index + 8) as *const u8 as *mut u8, // Note: don't write to this vector
                    256,
                    256,
                );
                inner_vec.push((Slice(key), Slice(value)));
            }

            SliceMmap {
                _inner_mmap: mmap,
                inner_vec: Some(inner_vec),
            }
        }
    }
}

impl SearchIndex for SliceMmap {
    fn len(&self) -> usize {
        match &self.inner_vec {
            Some(inner_vec) => inner_vec.len(),
            None => unreachable!(),
        }
    }
}

impl Index<std::ops::Range<usize>> for SliceMmap {
    type Output = [(Slice, Slice)];

    fn index(&self, index: Range<usize>) -> &Self::Output {
        match &self.inner_vec {
            Some(inner_vec) => inner_vec.index(index),
            None => unreachable!(),
        }
    }
}

impl Index<usize> for SliceMmap {
    type Output = (Slice, Slice);

    fn index(&self, index: usize) -> &Self::Output {
        match &self.inner_vec {
            Some(inner_vec) => inner_vec.index(index),
            None => unreachable!(),
        }
    }
}

quick_error! {
    #[derive(Debug)]
    pub enum SSTableError {
        FsError(err: agilulf_fs::FSError) {
            from()
        }
        IoError(err: std::io::Error) {
            from()
        }
    }
}
pub type SSTableResult<T> = std::result::Result<T, SSTableError>;

impl SSTable {
    fn freeze(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        let len = self.kv_pairs.len();

        for (key, value) in self.kv_pairs[0..len].iter() {
            buf.extend_from_slice(&key.0);
            buf.extend(vec![0; 8 - key.0.len()].iter());
            buf.extend_from_slice(&value.0);
            buf.extend(vec![0; 256 - value.0.len()].iter());
        }

        buf
    }

    pub async fn save<'a>(&'a self, path: &'a str) -> SSTableResult<()> {
        use agilulf_fs::File;
        let file = File::open(path)?;

        let buf = self.freeze();
        file.write(0, buf.as_slice()).await?;

        Ok(())
    }

    pub fn open(file: std::fs::File) -> SSTableResult<Self> {
        let mmap = box SliceMmap::from_mmap(unsafe { MmapOptions::new().map(&file)? });

        Ok(Self { kv_pairs: mmap })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn save_sstable() {
        let db = MemDatabase::default();
        SyncDatabase::put_sync(&db, Slice(b"HELLO".to_vec()), Slice(b"WORLD".to_vec())).unwrap();

        let sstable: SSTable = db.into();
        futures::executor::block_on(async move {
            sstable.save("/tmp/test_table").await.unwrap();
        });

        let mut reader = std::fs::File::open("/tmp/test_table").unwrap();
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).unwrap();

        assert_eq!(&buf[0..5], b"HELLO");
        assert_eq!(&buf[5..8], vec![0; 3].as_slice());
        assert_eq!(&buf[8..13], b"WORLD");
        assert_eq!(&buf[13..(256 + 8)], vec![0; 251].as_slice());
    }

    #[test]
    fn read_sstable() {
        let db = MemDatabase::default();
        SyncDatabase::put_sync(&db, Slice(b"HELLO".to_vec()), Slice(b"WORLD".to_vec())).unwrap();

        let sstable: SSTable = db.into();
        futures::executor::block_on(async move {
            sstable.save("/tmp/test_table").await.unwrap();
        });

        let file = std::fs::File::open("/tmp/test_table").unwrap();
        let sstable = SSTable::open(file).unwrap();
        let value = SyncDatabase::get_sync(&sstable, Slice(b"HELLO\0\0\0".to_vec())).unwrap();

        assert_eq!(&value.0[0..5], b"WORLD");
    }
}
