use std::sync::atomic::{AtomicPtr, Ordering};

pub trait LinkNode<T>: Sized {
    fn new(key: &T) -> *mut Self;

    fn get_prev(&self) -> AtomicPtr<Self>;

    fn get_succ(&self) -> AtomicPtr<Self>;

    fn get_key(&self) -> &T;

    fn set_next(&mut self, next: &AtomicPtr<Self>);
}

pub trait LinkList<T>: Sized
where
    T: std::cmp::PartialOrd,
{
    type N: LinkNode<T>;

    fn get_head(&self) -> AtomicPtr<Self::N>;

    fn insert(&self, key: &T) {
        let new_node = Self::N::new(key);

        unsafe {
            let mut succ = self.seek_greater(key);
            let mut prev = (*succ.load(Ordering::Relaxed)).get_prev();
            (*new_node).set_next(&succ);

            loop {
                if (*prev.load(Ordering::Relaxed))
                    .get_succ()
                    .compare_exchange(
                        succ.load(Ordering::Relaxed),
                        new_node,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    return;
                } else {
                    succ = self.seek_greater(key);
                    prev = (*succ.load(Ordering::Relaxed)).get_prev();
                };
            }
        }
    }

    fn seek_greater(&self, key: &T) -> AtomicPtr<Self::N> {
        unsafe {
            let mut now = self.get_head();

            while (*now.load(Ordering::Relaxed)).get_key() < key {
                now = (*now.load(Ordering::Relaxed)).get_succ();
            }

            now
        }
    }
}
