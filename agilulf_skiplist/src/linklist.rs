use std::sync::atomic::{AtomicPtr, Ordering};

pub trait LinkNode<T>: Sized {
    fn new(key: &T) -> *mut Self;

    fn get_succ(&self) -> &AtomicPtr<Self>;

    fn get_key(&self) -> &T;

    fn set_next(&mut self, next: *mut Self);
}

pub trait LinkList<T>: Sized
where
    T: std::cmp::PartialOrd,
{
    type Node: LinkNode<T>;

    fn get_head(&self) -> &AtomicPtr<Self::Node>;

    fn insert(&self, key: &T) {
        let new_node = Self::Node::new(key);

        unsafe {
            let (mut prev, mut succ) = self.seek(key);

            loop {
                (*new_node).set_next(succ.load(Ordering::SeqCst));

                if (*prev.load(Ordering::SeqCst))
                    .get_succ()
                    .compare_exchange(
                        succ.load(Ordering::SeqCst),
                        new_node,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    )
                    .is_ok()
                {
                    return;
                } else {
                    let res = self.seek_from(key, prev);
                    prev = res.0;
                    succ = res.1;
                };
            }
        }
    }

    fn seek_from<'a>(
        &self,
        key: &T,
        from: &'a AtomicPtr<Self::Node>,
    ) -> (&'a AtomicPtr<Self::Node>, &'a AtomicPtr<Self::Node>) {
        unsafe {
            let mut now: &AtomicPtr<Self::Node> = from;
            let mut next: &AtomicPtr<Self::Node> = (*from.load(Ordering::SeqCst)).get_succ();

            while (*next.load(Ordering::SeqCst)).get_key() < key {
                now = next;
                next = (*next.load(Ordering::SeqCst)).get_succ();
            }

            (now, next)
        }
    }

    fn seek<'a>(&self, key: &T) -> (&AtomicPtr<Self::Node>, &AtomicPtr<Self::Node>) {
        self.seek_from(key, self.get_head())
    }
}
