use super::non_standard_slice::{NonStandard, NonStandardSlice};
use super::skiplist::SkipList;
use agilulf_protocol::Slice;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

#[derive(Clone)]
struct Item {
    pub key: NonStandardSlice,
    pub value: NonStandardSlice,
    pub serial_number: u64,
}

impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.key < other.key {
            Some(std::cmp::Ordering::Less)
        } else if self.key > other.key {
            Some(std::cmp::Ordering::Greater)
        } else {
            if self.serial_number < other.serial_number {
                Some(std::cmp::Ordering::Less)
            } else if self.serial_number > other.serial_number {
                Some(std::cmp::Ordering::Greater)
            } else {
                Some(std::cmp::Ordering::Equal)
            }
        }
    }
}

impl NonStandard for Item {
    fn min() -> Self {
        Item {
            key: NonStandardSlice::MIN,
            value: NonStandardSlice::MIN,
            serial_number: std::u64::MIN,
        }
    }

    fn max() -> Self {
        Item {
            key: NonStandardSlice::MAX,
            value: NonStandardSlice::MAX,
            serial_number: std::u64::MAX,
        }
    }
}

impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.partial_cmp(other) == Some(std::cmp::Ordering::Equal)
    }
}

pub struct SkipMap {
    skiplist: SkipList<Item>,
    serial_number: AtomicU64,
}

impl SkipMap {
    fn new(serial_number: u64) -> SkipMap {
        SkipMap {
            skiplist: SkipList::new(),
            serial_number: AtomicU64::new(serial_number),
        }
    }

    fn insert(&self, key: &Slice, value: &Slice) {
        let new_item = Item {
            key: NonStandardSlice::Slice(key.clone()),
            value: NonStandardSlice::Slice(value.clone()),
            serial_number: self.serial_number.fetch_add(1, Ordering::SeqCst),
        };

        self.skiplist.insert(&new_item);
    }

    fn find(&self, key: &Slice) -> Option<Slice> {
        let new_item = Item {
            key: NonStandardSlice::Slice(key.clone()),
            value: NonStandardSlice::Slice(Slice(Vec::with_capacity(0))),
            serial_number: std::u64::MAX,
        };

        let (prev, next) = self.skiplist.find_key(&new_item);
        match &prev.key {
            NonStandardSlice::Slice(slice) => {
                if slice == key {
                    match &prev.value {
                        NonStandardSlice::Slice(value) => Some(value.clone()),
                        _ => unreachable!(),
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SkipMap;
    use agilulf_protocol::Slice;
    use rand::distributions::Standard;
    use rand::{thread_rng, Rng};
    use std::thread;
    use std::thread::JoinHandle;

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
    fn simple_map_test() {
        let map = SkipMap::new(0);

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

        let map = Arc::new(SkipMap::new(0));

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
}
