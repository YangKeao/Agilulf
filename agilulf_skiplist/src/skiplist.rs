use super::linklist::LinkList;
use super::linklist::LinkNode;
use super::non_standard_slice::NonStandard;
use rand::Rng;
use std::cmp::min;
use std::ptr::{null, null_mut};
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
        unsafe {
            let now = AtomicPtr::new(self as *const SkipListNode<T> as *mut SkipListNode<T>);
            let partial_skiplist = PartialSkipList::new(&now);
            let ret = partial_skiplist.seek(key);

            (ret.0.load(Ordering::AcqRel), ret.1.load(Ordering::AcqRel))
        }
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
        self.succ.store(next, Ordering::AcqRel);
    }
}

pub struct PartialSkipList<'a, T>
where
    T: std::cmp::PartialOrd,
{
    node: &'a AtomicPtr<SkipListNode<T>>,
}

impl<'a, T: std::cmp::PartialOrd> PartialSkipList<'a, T> {
    fn new(node: &'a AtomicPtr<SkipListNode<T>>) -> Self {
        Self { node }
    }

    fn base_level(&self) -> bool {
        unsafe { (*self.node.load(Ordering::AcqRel)).down.is_none() }
    }

    fn down_stairs(&self) -> Option<PartialSkipList<'a, T>> {
        unsafe {
            (*self.node.load(Ordering::AcqRel))
                .down
                .as_ref()
                .and_then(|node| Some(PartialSkipList { node }))
        }
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
    tail: Vec<AtomicPtr<SkipListNode<T>>>,
}

fn generate_level() -> usize {
    let mut rng = rand::thread_rng();
    let mut level = 1;

    while rng.gen() {
        level += 1;
    }

    return level;
}

impl<T: std::cmp::PartialOrd + Clone + NonStandard> SkipList<T> {
    fn new() -> SkipList<T> {
        let mut head: Vec<AtomicPtr<SkipListNode<T>>> = Vec::with_capacity(SKIPLIST_MAX_LEVEL);
        let mut tail: Vec<AtomicPtr<SkipListNode<T>>> = Vec::with_capacity(SKIPLIST_MAX_LEVEL);

        for index in 0..SKIPLIST_MAX_LEVEL {
            let head_node = SkipListNode::new(&T::min());
            let tail_node = SkipListNode::new(&T::max());

            unsafe {
                (*head_node).set_next(tail_node);

                if index > 0 {
                    (*head_node).set_down(head[index - 1].load(Ordering::AcqRel));
                    (*tail_node).set_down(tail[index - 1].load(Ordering::AcqRel));
                }
            }

            head.push(AtomicPtr::new(head_node));
            tail.push(AtomicPtr::new(tail_node));
        }
        SkipList { head, tail }
    }

    fn seek(&self, key: &T) -> Vec<(*mut SkipListNode<T>, *mut SkipListNode<T>)> {
        let head = self.head.last().unwrap(); // It's safe here
        let mut partial_skip_list = PartialSkipList::new(head);

        let mut ret = Vec::new();

        loop {
            let (prev, succ) = partial_skip_list.seek(key);
            ret.push((prev.load(Ordering::AcqRel), succ.load(Ordering::AcqRel)));

            if partial_skip_list.base_level() {
                return ret;
            } else {
                partial_skip_list = partial_skip_list.down_stairs().unwrap(); // It's safe here
            }
        }
    }

    fn insert(&self, key: &T) {
        let seek_result = self.seek(key);
        let level = min(generate_level(), SKIPLIST_MAX_LEVEL);

        let mut prev_level = null_mut();
        for index in 0..level {
            let new_node = SkipListNode::new(key);
            if index > 0 {
                unsafe { (*new_node).set_down(prev_level) };
            }

            let (mut prev, mut succ) = seek_result[index];

            loop {
                unsafe {
                    (*new_node).set_next(succ);

                    if (*prev)
                        .get_succ()
                        .compare_exchange(succ, new_node, Ordering::AcqRel, Ordering::AcqRel)
                        .is_ok()
                    {
                        prev_level = new_node;
                        break;
                    } else {
                        let res = unsafe { (*prev).seek(key) };
                        prev = res.0;
                        succ = res.1;
                    }
                }
            }
        }
    }
}
