//! This is a lock-free skiplist. The implementation of it is simple enough. Most of the implementation
//! learnt from [Lock-Free Linked Lists and Skip Lists](http://www.cse.yorku.ca/~ruppert/papers/lfll.pdf)
//!
//! But this implementation is quite simpler: it doesn't support delete. The resource will be freed
//! when the skiplist dropped. Without delete I don't have to face some famous "bugs" in lock-free
//! programming such as ABA problem.
//!
//! **Yeah! No epoch! No Hazard Pointer!**
//!
//! A better (maybe) implementation of skiplist is crossbeam-skiplist. However it is much more complicated
//! and is not released now (and I cannot wait for it)

#![feature(box_syntax)]

mod linklist;

mod skiplist;

mod non_standard_slice;

mod skipmap;

pub use skipmap::SkipMap;
