use agilulf_protocol::Slice;
use std::cmp::Ordering;
use std::iter::Peekable;

pub struct MergeIter<T: Iterator<Item = (Slice, Slice)>> {
    iters: Vec<Peekable<T>>,
}

impl<T: Iterator<Item = (Slice, Slice)>> Iterator for MergeIter<T> {
    type Item = (Slice, Slice);

    fn next(&mut self) -> Option<Self::Item> {
        let mut tmp_ret: Option<(Slice, Slice)> = None;
        let mut min_index = 0;
        for (index, iter) in self.iters.iter_mut().enumerate() {
            let item = iter.peek();
            if item.is_some() {
                let item = item.unwrap();
                if tmp_ret.is_none() {
                    tmp_ret = Some((item.0.clone(), item.1.clone()));
                    min_index = index;
                } else {
                    let cmp = tmp_ret.as_ref().unwrap().cmp(item);
                    match cmp {
                        Ordering::Less => {}
                        Ordering::Equal => {
                            while iter.peek().is_some()
                                && (iter.peek().unwrap() == tmp_ret.as_ref().unwrap())
                            {
                                iter.next();
                            }
                            continue;
                        }
                        Ordering::Greater => {
                            tmp_ret = Some((item.0.clone(), item.1.clone()));
                            min_index = index;
                        }
                    }
                }
            }
        }

        while self.iters[min_index].peek().is_some()
            && (self.iters[min_index].peek().unwrap() == tmp_ret.as_ref().unwrap())
        {
            self.iters[min_index].next();
        }

        tmp_ret
    }
}

pub fn merge_iter<T>(iters: Vec<T>) -> MergeIter<T>
where
    T: Iterator<Item = (Slice, Slice)>,
{
    MergeIter {
        iters: iters.into_iter().map(|item| item.peekable()).collect(),
    }
}
