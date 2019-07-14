use agilulf_protocol::Slice;
use std::cmp::Ordering;

pub trait NonStandard: PartialOrd {
    fn min() -> Self;
    fn max() -> Self;
}

#[derive(Clone, Eq, Debug)]
pub enum NonStandardSlice {
    MIN,
    Slice(Slice),
    MAX,
}

impl PartialOrd for NonStandardSlice {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self {
            NonStandardSlice::MIN => match other {
                NonStandardSlice::MIN => Some(Ordering::Equal),
                _ => Some(Ordering::Less),
            },
            NonStandardSlice::MAX => match other {
                NonStandardSlice::MAX => Some(Ordering::Equal),
                _ => Some(Ordering::Greater),
            },
            NonStandardSlice::Slice(slice) => match other {
                NonStandardSlice::MIN => Some(Ordering::Greater),
                NonStandardSlice::MAX => Some(Ordering::Less),
                NonStandardSlice::Slice(other) => slice.partial_cmp(&other),
            },
        }
    }
}

impl PartialEq for NonStandardSlice {
    fn eq(&self, other: &Self) -> bool {
        self.partial_cmp(other) == Some(Ordering::Equal)
    }
}

impl Ord for NonStandardSlice {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap() // here it is safe.
    }
}

impl NonStandard for NonStandardSlice {
    fn min() -> Self {
        NonStandardSlice::MIN
    }

    fn max() -> Self {
        NonStandardSlice::MAX
    }
}
