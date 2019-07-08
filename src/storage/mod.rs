pub struct Value {}

pub trait MemTable {
    fn new() -> Self;

    fn put(key: Value, val: Value);
    fn size() -> usize;
    fn freeze() -> Self;
}

pub struct InternalDB<M: MemTable> {
    memtable: M,
}

impl<M: MemTable> InternalDB<M> {
    fn new() -> Self {
        InternalDB { memtable: M::new() }
    }
}
