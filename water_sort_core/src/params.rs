//! Puzzle parameters and their validation.

use serde::{Deserialize, Serialize};

/// Largest supported number of tubes (`n_colors + n_empty`).
pub const MAX_TUBES: usize = 16;
/// Largest supported tube capacity.
pub const MAX_CAP: usize = 8;

/// Shape of a puzzle: how many colors, how tall the tubes are, and how many extra empty tubes.
///
/// There are `n_colors * capacity` units in total, `capacity` of each color, spread over
/// `n_colors + n_empty` tubes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Params {
    pub n_colors: u8,
    pub capacity: u8,
    pub n_empty: u8,
}

/// Why a [`Params`] value is rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ParamsError {
    #[error("n_colors must be at least 1")]
    NoColors,
    #[error("capacity {capacity} is below the minimum of 2")]
    CapacityTooSmall { capacity: u8 },
    #[error("capacity {capacity} exceeds the maximum of {MAX_CAP}")]
    CapacityTooLarge { capacity: u8 },
    #[error("{n_tubes} tubes exceed the maximum of {MAX_TUBES}")]
    TooManyTubes { n_tubes: usize },
}

impl Params {
    pub const DEFAULT_CAPACITY: u8 = 4;
    pub const DEFAULT_N_EMPTY: u8 = 2;

    /// Parameters with the default capacity and number of empty tubes. Not validated.
    pub const fn with_colors(n_colors: u8) -> Self {
        Self {
            n_colors,
            capacity: Self::DEFAULT_CAPACITY,
            n_empty: Self::DEFAULT_N_EMPTY,
        }
    }

    /// Number of tubes, `n_colors + n_empty`.
    pub const fn n_tubes(&self) -> usize {
        self.n_colors as usize + self.n_empty as usize
    }

    /// Total number of units, `n_colors * capacity`.
    pub const fn n_units(&self) -> usize {
        self.n_colors as usize * self.capacity as usize
    }

    /// Checks the parameters against the fixed-size limits of [`crate::State`].
    ///
    /// # Errors
    ///
    /// Returns the first violated constraint.
    pub const fn validate(&self) -> Result<(), ParamsError> {
        if self.n_colors == 0 {
            return Err(ParamsError::NoColors);
        }
        if self.capacity < 2 {
            return Err(ParamsError::CapacityTooSmall {
                capacity: self.capacity,
            });
        }
        if self.capacity as usize > MAX_CAP {
            return Err(ParamsError::CapacityTooLarge {
                capacity: self.capacity,
            });
        }
        if self.n_tubes() > MAX_TUBES {
            return Err(ParamsError::TooManyTubes {
                n_tubes: self.n_tubes(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn p(n_colors: u8, capacity: u8, n_empty: u8) -> Params {
        Params {
            n_colors,
            capacity,
            n_empty,
        }
    }

    #[test]
    fn defaults() {
        let params = Params::with_colors(5);
        assert_eq!(params, p(5, 4, 2));
        assert_eq!(params.n_tubes(), 7);
        assert_eq!(params.n_units(), 20);
        assert_eq!(params.validate(), Ok(()));
    }

    #[test]
    fn validation_branches() {
        assert_eq!(p(0, 4, 2).validate(), Err(ParamsError::NoColors));
        assert_eq!(
            p(3, 1, 2).validate(),
            Err(ParamsError::CapacityTooSmall { capacity: 1 })
        );
        assert_eq!(
            p(3, 9, 2).validate(),
            Err(ParamsError::CapacityTooLarge { capacity: 9 })
        );
        assert_eq!(
            p(15, 4, 2).validate(),
            Err(ParamsError::TooManyTubes { n_tubes: 17 })
        );
        assert_eq!(p(14, 8, 2).validate(), Ok(()));
        assert_eq!(p(1, 2, 0).validate(), Ok(()));
        assert_eq!(p(16, 2, 0).validate(), Ok(()));
    }
}
