use super::non_standard_slice::{NonStandard, NonStandardSlice};
use super::skiplist::SkipList;
use agilulf_protocol::Slice;
use std::ops::RangeBounds;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

#[derive(Clone)]
struct Item<T: Default + Clone> {
    pub key: NonStandardSlice,
    pub value: T,
    pub serial_number: u64,
}

impl<T: Default + Clone> PartialOrd for Item<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.key < other.key {
            Some(std::cmp::Ordering::Less)
        } else if self.key > other.key {
            Some(std::cmp::Ordering::Greater)
        } else if self.serial_number < other.serial_number {
            Some(std::cmp::Ordering::Greater)
        } else if self.serial_number > other.serial_number {
            Some(std::cmp::Ordering::Less)
        } else {
            Some(std::cmp::Ordering::Equal)
        }
    }
}

impl<T: Default + Clone> NonStandard for Item<T> {
    fn min() -> Self {
        Item {
            key: NonStandardSlice::MIN,
            value: T::default(),
            serial_number: std::u64::MAX,
        }
    }

    fn max() -> Self {
        Item {
            key: NonStandardSlice::MAX,
            value: T::default(),
            serial_number: std::u64::MIN,
        }
    }
}

impl<T: Default + Clone> PartialEq for Item<T> {
    fn eq(&self, other: &Self) -> bool {
        self.partial_cmp(other) == Some(std::cmp::Ordering::Equal)
    }
}

/// A map contains a skiplist and a serial_number.
///
/// ```ignore
/// pub struct SkipMap<T: Default + Clone> {
///     skiplist: SkipList<Item<T>>,
///     serial_number: AtomicU64,
/// }
/// ```
///
/// `serial_number` is automatically increased. It's used to keep the order of insert: The later it
/// is inserted, the smaller it is. (Actually the `serial_number` is bigger but the item is smaller
/// according to the strategy of comparing item.
///
/// Generic parameter `T` should be `Default + Clone`. However the limitation can be relaxed to only
/// `Default`. The `Clone` here is to avoid lifetime parameter and keep this crate simple. (And
/// what we need to use for T is actually `Clone`). If we remove the `Clone` limitation, the `insert`
/// method may need to receive a `T` but not `&T`
pub struct SkipMap<T: Default + Clone> {
    skiplist: SkipList<Item<T>>,
    serial_number: AtomicU64,
}

impl<T: Default + Clone> Default for SkipMap<T> {
    fn default() -> Self {
        Self::new(0)
    }
}

impl<T: Default + Clone> SkipMap<T> {
    fn new(serial_number: u64) -> SkipMap<T> {
        SkipMap {
            skiplist: SkipList::default(),
            serial_number: AtomicU64::new(serial_number),
        }
    }

    pub fn len(&self) -> u64 {
        self.serial_number.load(Ordering::SeqCst)
    }

    ///```
    /// # use agilulf_skiplist::SkipMap;
    /// # use agilulf_protocol::Slice;
    /// let map: SkipMap<Slice> = SkipMap::default();
    /// map.insert(&Slice(b"key1".to_vec()), &Slice(b"value1".to_vec()));
    /// map.insert(&Slice(b"key2".to_vec()), &Slice(b"value2".to_vec()));
    /// map.insert(&Slice(b"key3".to_vec()), &Slice(b"value3".to_vec()));
    ///
    /// assert_eq!(
    ///     map.find(&Slice(b"key1".to_vec())).unwrap(),
    ///     Slice(b"value1".to_vec())
    /// );
    ///
    /// map.insert(
    ///     &Slice(b"key1".to_vec()),
    ///     &Slice(b"modified_value1".to_vec()),
    /// );
    /// assert_eq!(
    ///     map.find(&Slice(b"key1".to_vec())).unwrap(),
    ///     Slice(b"modified_value1".to_vec())
    /// );
    ///```
    pub fn insert(&self, key: &Slice, value: &T) {
        let new_item = Item {
            key: NonStandardSlice::Slice(key.clone()),
            value: value.clone(),
            serial_number: self.serial_number.fetch_add(1, Ordering::SeqCst),
        };

        self.skiplist.insert(&new_item);
    }

    pub fn find(&self, key: &Slice) -> Option<T> {
        let new_item = Item {
            key: NonStandardSlice::Slice(key.clone()),
            value: T::default(),
            serial_number: std::u64::MAX,
        };

        let item = self.skiplist.read_key(&new_item);
        match &item.key {
            NonStandardSlice::Slice(slice) => {
                if slice == key {
                    Some(item.value.clone())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn scan<R>(&self, range: R) -> Vec<(Slice, T)>
    where
        R: RangeBounds<Slice>,
    {
        use std::ops::Bound;

        let start_item = match range.start_bound() {
            Bound::Included(bound) => Item {
                key: NonStandardSlice::Slice(bound.clone()),
                value: T::default(),
                serial_number: std::u64::MAX,
            },
            Bound::Excluded(bound) => Item {
                key: NonStandardSlice::Slice(bound.clone()),
                value: T::default(),
                serial_number: std::u64::MIN,
            },
            Bound::Unbounded => Item::min(),
        };
        let end_item = match range.end_bound() {
            Bound::Included(bound) => Item {
                key: NonStandardSlice::Slice(bound.clone()),
                value: T::default(),
                serial_number: std::u64::MIN,
            },
            Bound::Excluded(bound) => Item {
                key: NonStandardSlice::Slice(bound.clone()),
                value: T::default(),
                serial_number: std::u64::MAX,
            },
            Bound::Unbounded => Item::max(),
        };
        let data = self
            .skiplist
            .scan(&start_item, &end_item)
            .into_iter()
            .map(|item| (&item.key, &item.value, &item.serial_number));

        let mut ret = Vec::new();
        let mut last_key = &NonStandardSlice::MIN;
        for (key, value, _) in data {
            if last_key != key {
                ret.push((key.clone().unwrap(), value.clone()));
                last_key = key;
            } else {
                continue;
            }
        }

        ret
    }
}

#[cfg(test)]
mod tests {
    use super::SkipMap;
    use agilulf_protocol::Slice;
    use rand::distributions::Standard;
    use rand::{thread_rng, Rng};
    use std::collections::BTreeMap;
    use std::thread;

    fn generate_keys(num: usize) -> Vec<Vec<u8>> {
        (0..num)
            .map(|_| thread_rng().sample_iter(&Standard).take(8).collect())
            .collect()
    }

    fn generate_values(num: usize) -> Vec<Vec<u8>> {
        (0..num)
            .map(|_| thread_rng().sample_iter(&Standard).take(256).collect())
            .collect()
    }

    #[test]
    fn simple_put_get_test() {
        let map: SkipMap<Slice> = SkipMap::new(0);
        map.insert(&Slice(b"key1".to_vec()), &Slice(b"value1".to_vec()));
        map.insert(&Slice(b"key2".to_vec()), &Slice(b"value2".to_vec()));
        map.insert(&Slice(b"key3".to_vec()), &Slice(b"value3".to_vec()));

        assert_eq!(
            map.find(&Slice(b"key1".to_vec())).unwrap(),
            Slice(b"value1".to_vec())
        );

        map.insert(
            &Slice(b"key1".to_vec()),
            &Slice(b"modified_value1".to_vec()),
        );
        assert_eq!(
            map.find(&Slice(b"key1".to_vec())).unwrap(),
            Slice(b"modified_value1".to_vec())
        );
    }

    #[test]
    fn simple_map_test() {
        let map: SkipMap<Slice> = SkipMap::new(0);

        let keys: Vec<Slice> = generate_keys(1000)
            .into_iter()
            .map(|key| Slice(key))
            .collect();
        let values: Vec<Slice> = generate_values(1000)
            .into_iter()
            .map(|key| Slice(key))
            .collect();

        for i in 0..1000 {
            map.insert(&keys[i], &values[i])
        }

        for i in 0..1000 {
            let value = map.find(&keys[i]).unwrap();
            assert_eq!(value, values[i]);
        }
    }

    #[test]
    fn multi_thread_test() {
        use std::sync::Arc;

        let map: Arc<SkipMap<Slice>> = Arc::new(SkipMap::new(0));

        let map_ref = &map;
        (0..4)
            .map(move |_| {
                let map = map_ref.clone();
                thread::spawn(move || {
                    let keys: Vec<Slice> = generate_keys(1000)
                        .into_iter()
                        .map(|key| Slice(key))
                        .collect();
                    let values: Vec<Slice> = generate_values(1000)
                        .into_iter()
                        .map(|key| Slice(key))
                        .collect();

                    for i in 0..1000 {
                        map.insert(&keys[i], &values[i])
                    }

                    for i in 0..1000 {
                        let value = map.find(&keys[i]).unwrap();
                        assert_eq!(value, values[i]);
                    }
                })
            })
            .for_each(|thread| thread.join().unwrap());
    }

    #[test]
    fn scan() {
        let map: SkipMap<Slice> = SkipMap::new(0);
        let mut btree_map = BTreeMap::new();

        let keys: Vec<Slice> = generate_keys(1000)
            .into_iter()
            .map(|key| Slice(key))
            .collect();
        let values: Vec<Slice> = generate_values(1000)
            .into_iter()
            .map(|key| Slice(key))
            .collect();

        for i in 0..1000 {
            map.insert(&keys[i], &values[i]);
            btree_map.insert(keys[i].clone(), values[i].clone());
        }

        for _ in 0..5 {
            let start = rand::thread_rng().gen_range(0, 1000);
            let end = rand::thread_rng().gen_range(start, 1000);

            let range_start = std::cmp::min(keys[start].clone(), keys[end].clone());
            let range_end = std::cmp::max(keys[start].clone(), keys[end].clone());

            let kv_pair: Vec<(Slice, Slice)> = map.scan(range_start.clone()..range_end.clone());
            let kv_pair_exp: Vec<(Slice, Slice)> = btree_map
                .range(range_start.clone()..range_end.clone())
                .into_iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();

            assert_eq!(kv_pair.len(), kv_pair_exp.len());
            for i in 0..kv_pair.len() {
                assert_eq!(kv_pair[i], kv_pair_exp[i]);
            }
        }
    }

    #[test]
    fn update_test() {
        let map: SkipMap<Slice> = SkipMap::default();
        map.insert(&Slice(b"key1".to_vec()), &Slice(b"value1".to_vec()));
        map.insert(&Slice(b"key2".to_vec()), &Slice(b"value2".to_vec()));
        map.insert(&Slice(b"key3".to_vec()), &Slice(b"value3".to_vec()));

        let scan_result = map.scan(Slice(b"key1".to_vec())..Slice(b"key3".to_vec()));
        assert_eq!(
            scan_result,
            vec![
                (Slice(b"key1".to_vec()), Slice(b"value1".to_vec())),
                (Slice(b"key2".to_vec()), Slice(b"value2".to_vec()))
            ]
        );

        map.insert(
            &Slice(b"key1".to_vec()),
            &Slice(b"modified_value1".to_vec()),
        );
        let scan_result = map.scan(Slice(b"key1".to_vec())..Slice(b"key3".to_vec()));
        assert_eq!(
            scan_result,
            vec![
                (Slice(b"key1".to_vec()), Slice(b"modified_value1".to_vec())),
                (Slice(b"key2".to_vec()), Slice(b"value2".to_vec()))
            ]
        );
    }
}
