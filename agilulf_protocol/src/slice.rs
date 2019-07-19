use std::cmp::Ordering;

use libc::memcmp;

/// In the most time of this project, the key or value will be represented by a so called Slice (actually
/// not a slice but a `Vec<u8>`).
///
/// It is not a good design (for both egineering and efficiency). But it's a compromise between programming
/// efficiency and runtime efficiency.
///
/// Good Part:
///
/// It avoids the battle with lifetime. With Future and async/await used in this project, complicated
/// lifetime should be avoided at most time. Or I will fall into the blackhole of lifetime contradiction.
///
/// Bad Part:
///
/// When facing the low level operation, or the memory is gotten from mmap, using vector may lead to
/// unexpected error. One of them is unexpected `free()`. While dropping a vector, it will automatically
/// free the memory owned by it. However, if I use `from_raw_parts` and make up a vector from memmap
/// memory, it will drop this memmaped memory and lead to a panic.
///
#[derive(Clone, Eq, Debug)]
pub struct Slice(pub Vec<u8>);

impl Default for Slice {
    fn default() -> Self {
        Slice(Vec::with_capacity(0))
    }
}

impl PartialOrd for Slice {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.0.len() < other.0.len() {
            Some(Ordering::Less)
        } else if self.0.len() > other.0.len() {
            Some(Ordering::Greater)
        } else {
            let res = unsafe {
                memcmp(
                    self.0.as_ptr() as *const core::ffi::c_void,
                    other.0.as_ptr() as *const core::ffi::c_void,
                    self.0.len(),
                )
            };
            if res == 0 {
                Some(Ordering::Equal)
            } else if res < 0 {
                Some(Ordering::Less)
            } else {
                Some(Ordering::Greater)
            }
        }
    }
}

impl PartialEq for Slice {
    fn eq(&self, other: &Self) -> bool {
        self.partial_cmp(other) == Some(Ordering::Equal)
    }
}

impl Ord for Slice {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap() // here it is safe.
    }
}
