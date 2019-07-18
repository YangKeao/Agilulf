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
            match item {
                Some(item) => match &tmp_ret {
                    None => {
                        tmp_ret = Some((item.0.clone(), item.1.clone()));
                        min_index = index;
                    }
                    Some(ret) => {
                        let cmp = ret.cmp(item);
                        match cmp {
                            Ordering::Less => {}
                            Ordering::Equal => {
                                while let Some(content) = iter.peek() {
                                    if content == ret {
                                        iter.next();
                                    } else {
                                        break;
                                    }
                                }
                                continue;
                            }
                            Ordering::Greater => {
                                tmp_ret = Some((item.0.clone(), item.1.clone()));
                                min_index = index;
                            }
                        }
                    }
                },
                None => {}
            }
        }

        while let Some(content) = self.iters[min_index].peek() {
            match tmp_ret.as_ref() {
                Some(ret) => {
                    if ret == content {
                        self.iters[min_index].next();
                    }
                }
                None => {
                    break;
                }
            }
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
