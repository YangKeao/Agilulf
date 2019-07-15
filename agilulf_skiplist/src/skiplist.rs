use super::linklist::LinkList;
use super::linklist::LinkNode;
use super::non_standard_slice::NonStandard;
use rand::Rng;
use std::cmp::min;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicPtr, Ordering};

const SKIPLIST_MAX_LEVEL: usize = 128;

pub struct SkipListNode<T> {
    key: T,
    succ: AtomicPtr<SkipListNode<T>>,
    down: Option<AtomicPtr<SkipListNode<T>>>,
}

impl<T: std::cmp::PartialOrd + Clone> SkipListNode<T> {
    fn set_down(&mut self, down: *mut SkipListNode<T>) {
        self.down = Some(AtomicPtr::new(down));
    }

    fn seek(&self, key: &T) -> (*mut SkipListNode<T>, *mut SkipListNode<T>) {
        let now = AtomicPtr::new(self as *const SkipListNode<T> as *mut SkipListNode<T>);
        let partial_skiplist = PartialSkipList::new(&now);
        let ret = partial_skiplist.seek(key);

        (ret.0.load(Ordering::SeqCst), ret.1.load(Ordering::SeqCst))
    }
}

impl<T: std::cmp::PartialOrd + Clone> LinkNode<T> for SkipListNode<T> {
    fn new(key: &T) -> *mut Self {
        Box::into_raw(box SkipListNode {
            key: key.clone(),
            succ: AtomicPtr::new(null_mut()),
            down: None,
        })
    }

    fn get_succ(&self) -> &AtomicPtr<Self> {
        &self.succ
    }

    fn get_key(&self) -> &T {
        &self.key
    }

    fn set_next(&mut self, next: *mut Self) {
        self.succ.store(next, Ordering::SeqCst);
    }
}

pub struct PartialSkipList<'a, T>
where
    T: std::cmp::PartialOrd,
{
    node: &'a AtomicPtr<SkipListNode<T>>,
}

impl<'a, T: std::cmp::PartialOrd> PartialSkipList<'a, T> {
    fn new(node: &'a AtomicPtr<SkipListNode<T>>) -> PartialSkipList<'a, T> {
        Self { node }
    }

    fn base_level(&self) -> bool {
        unsafe { (*self.node.load(Ordering::SeqCst)).down.is_none() }
    }
}

impl<T: std::cmp::PartialOrd + Clone> LinkList<T> for PartialSkipList<'_, T> {
    type Node = SkipListNode<T>;

    fn get_head(&self) -> &AtomicPtr<Self::Node> {
        self.node
    }
}

pub struct SkipList<T: std::cmp::PartialOrd + Clone + NonStandard> {
    head: Vec<AtomicPtr<SkipListNode<T>>>,
    _tail: Vec<AtomicPtr<SkipListNode<T>>>,
}

fn generate_level() -> usize {
    let mut rng = rand::thread_rng();
    let mut level = 1;

    while rng.gen() {
        level += 1;
    }

    level
}

impl<T: std::cmp::PartialOrd + Clone + NonStandard> Default for SkipList<T> {
    fn default() -> SkipList<T> {
        let mut head: Vec<AtomicPtr<SkipListNode<T>>> = Vec::with_capacity(SKIPLIST_MAX_LEVEL);
        let mut tail: Vec<AtomicPtr<SkipListNode<T>>> = Vec::with_capacity(SKIPLIST_MAX_LEVEL);

        for index in 0..SKIPLIST_MAX_LEVEL {
            let head_node = SkipListNode::new(&T::min());
            let tail_node = SkipListNode::new(&T::max());

            unsafe {
                (*head_node).set_next(tail_node);

                if index > 0 {
                    (*head_node).set_down(head[index - 1].load(Ordering::SeqCst));
                    (*tail_node).set_down(tail[index - 1].load(Ordering::SeqCst));
                }
            }

            head.push(AtomicPtr::new(head_node));
            tail.push(AtomicPtr::new(tail_node));
        }
        SkipList { head, _tail: tail }
    }
}

impl<T: std::cmp::PartialOrd + Clone + NonStandard> SkipList<T> {
    fn seek(&self, key: &T) -> Vec<(*mut SkipListNode<T>, *mut SkipListNode<T>)> {
        let head = self.head.last().unwrap(); // It's safe here
        let mut partial_skip_list = PartialSkipList::new(head);

        let mut ret = Vec::new();

        loop {
            let prev_ptr = {
                let (prev, succ) = partial_skip_list.seek(key);
                ret.push((prev.load(Ordering::SeqCst), succ.load(Ordering::SeqCst)));
                prev.load(Ordering::SeqCst)
            };

            if partial_skip_list.base_level() {
                return ret;
            } else {
                unsafe {
                    partial_skip_list = PartialSkipList::new((*prev_ptr).down.as_ref().unwrap())
                }
            }
        }
    }

    pub fn find_key(&self, key: &T) -> (*mut SkipListNode<T>, *mut SkipListNode<T>) {
        let head = self.head.last().unwrap(); // It's safe here
        let mut partial_skip_list = PartialSkipList::new(head);

        loop {
            let (prev_ptr, next_ptr) = {
                let (prev, succ) = partial_skip_list.seek(key);
                (prev.load(Ordering::SeqCst), succ.load(Ordering::SeqCst))
            };

            if partial_skip_list.base_level() {
                return (prev_ptr, next_ptr);
            } else {
                unsafe {
                    partial_skip_list = PartialSkipList::new((*prev_ptr).down.as_ref().unwrap())
                }
            }
        }
    }

    pub fn read_key(&self, key: &T) -> &T {
        let (prev, _) = self.find_key(key);
        unsafe { (*prev).get_key() }
    }

    pub fn scan(&self, start: &T, end: &T) -> Vec<&T> {
        let (_, mut next) = self.find_key(start);
        let mut ret = Vec::new();

        unsafe {
            while (*next).get_key() < end {
                ret.push((*next).get_key());
                next = (*next).get_succ().load(Ordering::SeqCst);
            }
        }
        ret
    }

    pub fn insert(&self, key: &T) {
        let seek_result = self.seek(key);
        let level = min(generate_level(), SKIPLIST_MAX_LEVEL);

        let mut prev_level = null_mut();
        for index in 0..level {
            let new_node = SkipListNode::new(key);
            if index > 0 {
                unsafe { (*new_node).set_down(prev_level) };
            }

            let (mut prev, mut succ) = seek_result[seek_result.len() - index - 1];

            loop {
                unsafe {
                    (*new_node).set_next(succ);

                    if (*prev)
                        .get_succ()
                        .compare_exchange(succ, new_node, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
                    {
                        prev_level = new_node;
                        break;
                    } else {
                        let res = (*prev).seek(key);
                        prev = res.0;
                        succ = res.1;
                    }
                }
            }
        }
    }
}

impl<T: std::cmp::PartialOrd + Clone + NonStandard> Drop for SkipList<T> {
    fn drop(&mut self) {
        let mut now = self.head[0].load(Ordering::SeqCst);
        while unsafe { (*now).get_key() < T::max() } {
            let now_ptr = now.clone();
            let next_ptr = unsafe { (*now).get_succ().load(Ordering::SeqCst) };

            unsafe {
                drop(Box::from_raw(now_ptr));
            }
            now = next_ptr;
        }
        unsafe {
            drop(Box::from_raw(now));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl NonStandard for i32 {
        fn min() -> Self {
            std::i32::MIN
        }

        fn max() -> Self {
            std::i32::MAX
        }
    }

    #[test]
    fn skiplist_basic_test() {
        let skiplist: SkipList<i32> = SkipList::new();

        for i in 0..100 {
            skiplist.insert(&(i * 2));
        }

        for i in 0..100 {
            let prev = skiplist.read_key(&(i * 2));

            if i == 0 {
                assert_eq!(prev, &std::i32::MIN);
            } else {
                assert_eq!(prev, &((i - 1) * 2));
            }
        }
    }
}
