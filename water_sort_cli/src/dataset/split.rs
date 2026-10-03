//! Train / val / test split by canonical hash: `bucket = canonical_hash % 100`.
//!
//! The hash is canonical (D8), so every tube and color permutation of a puzzle lands in the same
//! split, and within one dataset a test puzzle can never reappear in train.

use core::fmt;
use core::str::FromStr;

use serde::{Deserialize, Serialize};

/// Which part of a dataset a record belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Split {
    Train,
    Val,
    Test,
}

impl Split {
    pub const ALL: [Self; 3] = [Self::Train, Self::Val, Self::Test];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Train => "train",
            Self::Val => "val",
            Self::Test => "test",
        }
    }
}

impl fmt::Display for Split {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Split {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|x| x.name() == s)
            .ok_or_else(|| format!("unknown split {s:?} (expected train, val or test)"))
    }
}

/// Bucket ranges: `[0, train_end)` train, `[train_end, val_end)` val, `[val_end, 100)` test.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SplitRanges {
    pub train_end: u8,
    pub val_end: u8,
}

impl SplitRanges {
    /// 80 / 10 / 10.
    pub const DEFAULT: Self = Self {
        train_end: 80,
        val_end: 90,
    };

    /// The bucket of a canonical hash, `0..100`.
    pub const fn bucket(canonical_hash: u64) -> u8 {
        (canonical_hash % 100) as u8
    }

    /// The split of a canonical hash.
    pub const fn of(&self, canonical_hash: u64) -> Split {
        let b = Self::bucket(canonical_hash);
        if b < self.train_end {
            Split::Train
        } else if b < self.val_end {
            Split::Val
        } else {
            Split::Test
        }
    }

    /// `[start, end)` bucket range of `split`.
    pub const fn range(&self, split: Split) -> (u8, u8) {
        match split {
            Split::Train => (0, self.train_end),
            Split::Val => (self.train_end, self.val_end),
            Split::Test => (self.val_end, 100),
        }
    }
}

impl Default for SplitRanges {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// `train,val,test` percentages summing to 100, e.g. `80,10,10`.
impl FromStr for SplitRanges {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let parts: Vec<u8> = s
            .split(',')
            .map(|p| p.trim().parse::<u8>())
            .collect::<Result<_, _>>()
            .map_err(|e| format!("invalid split {s:?}: {e}"))?;
        let [train, val, test] = parts[..] else {
            return Err(format!(
                "split {s:?} needs three percentages train,val,test"
            ));
        };
        if u16::from(train) + u16::from(val) + u16::from(test) != 100 {
            return Err(format!("split {s:?} does not sum to 100"));
        }
        Ok(Self {
            train_end: train,
            val_end: train + val,
        })
    }
}

impl fmt::Display for SplitRanges {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{},{},{}",
            self.train_end,
            self.val_end - self.train_end,
            100 - self.val_end
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets() {
        let r = SplitRanges::DEFAULT;
        assert_eq!(r.of(0), Split::Train);
        assert_eq!(r.of(79), Split::Train);
        assert_eq!(r.of(180), Split::Val);
        assert_eq!(r.of(89), Split::Val);
        assert_eq!(r.of(90), Split::Test);
        assert_eq!(r.of(u64::MAX), Split::Train); // u64::MAX % 100 == 15
        assert_eq!(r.range(Split::Val), (80, 90));
        assert_eq!("80,10,10".parse(), Ok(r));
        assert_eq!(r.to_string(), "80,10,10");
        assert_eq!(
            "70, 0, 30".parse(),
            Ok(SplitRanges {
                train_end: 70,
                val_end: 70
            })
        );
        assert!("80,10".parse::<SplitRanges>().is_err());
        assert!("80,10,20".parse::<SplitRanges>().is_err());
        assert_eq!("test".parse(), Ok(Split::Test));
        assert!("dev".parse::<Split>().is_err());
    }
}
