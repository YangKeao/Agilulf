use super::{mem_database::MemDatabase, AsyncDatabase, SyncDatabase};
use agilulf_protocol::error::database_error::{DatabaseError, Result};
use agilulf_protocol::Slice;
use std::cmp::Ordering;
use std::ops::Index;

pub trait SearchIndex:
    Index<usize, Output = (Slice, Slice)>
    + Index<std::ops::Range<usize>, Output = [(Slice, Slice)]>
    + Sync
    + Send
{
    fn len(&self) -> usize;

    // TODO: this implementation of binary_search may fall in left or right. Make it stable in the future.
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
}

#[repr(packed)]
pub struct SSTable {
    kv_pairs: Box<dyn SearchIndex>,
}

impl SyncDatabase for SSTable {
    fn get(&self, key: Slice) -> Result<Slice> {
        let index = self.kv_pairs.binary_search_by_key(&key);

        Ok(self.kv_pairs[index].1.clone())
    }

    fn put(&self, _: Slice, _: Slice) -> Result<()> {
        panic!("Cannot modify SSTable")
    }

    fn scan(&self, start: Slice, end: Slice) -> Vec<(Slice, Slice)> {
        let start_index = self.kv_pairs.binary_search_by_key(&start);
        let end_index = self.kv_pairs.binary_search_by_key(&end);
        self.kv_pairs[start_index..end_index].to_vec()
    }

    fn delete(&self, _: Slice) -> Result<()> {
        panic!("Cannot modify SSTable")
    }
}

impl SearchIndex for Vec<(Slice, Slice)> {
    fn len(&self) -> usize {
        Vec::len(self)
    }
}

impl From<MemDatabase> for SSTable {
    fn from(mem_database: MemDatabase) -> Self {
        let kv_pairs: Box<Vec<(Slice, Slice)>> =
            box SyncDatabase::scan(&mem_database, Slice(vec![0; 8]), Slice(vec![255; 8]))
                .into_iter()
                .collect();

        Self { kv_pairs }
    }
}
