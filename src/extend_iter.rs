pub trait ExtendIter<T>: Iterator {
    fn seek(&mut self, val: &T) -> Option<Self::Item>;
}
