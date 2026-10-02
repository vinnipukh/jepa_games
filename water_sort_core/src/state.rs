//! Puzzle state: fixed-size tube arrays, validation, and solved detection.

use core::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::params::{MAX_CAP, MAX_TUBES, Params, ParamsError};

/// Marker for a cell that holds no unit.
pub const EMPTY: u8 = u8::MAX;

/// A full puzzle position.
///
/// Tubes are stored bottom to top. Cells above a tube's fill height are [`EMPTY`], and tubes at
/// index `n_tubes` and beyond are entirely [`EMPTY`], so derived equality and hashing only see
/// meaningful data.
///
/// Every `State` holds exactly `capacity` units of each color `0..n_colors`; constructors reject
/// anything else.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct State {
    params: Params,
    cells: [[u8; MAX_CAP]; MAX_TUBES],
    heights: [u8; MAX_TUBES],
}

/// Why a tube layout is not a valid [`State`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StateError {
    #[error(transparent)]
    Params(#[from] ParamsError),
    #[error("expected {expected} tubes, got {got}")]
    TubeCount { expected: usize, got: usize },
    #[error("tube {tube} holds {len} units, capacity is {capacity}")]
    TubeOverflow {
        tube: usize,
        len: usize,
        capacity: u8,
    },
    #[error("tube {tube} contains color {color}, which is out of range")]
    InvalidColor { tube: usize, color: u8 },
    #[error("color {color} occurs {count} times, expected {expected}")]
    ColorCount {
        color: u8,
        count: usize,
        expected: u8,
    },
    #[error("expected {expected} units, got {got}")]
    UnitCount { expected: usize, got: usize },
}

impl State {
    /// Builds a state from explicit tube contents (bottom to top).
    ///
    /// # Errors
    ///
    /// Rejects invalid params, a wrong tube count, overfull tubes, out-of-range colors, and any
    /// color that does not occur exactly `capacity` times.
    pub fn from_tubes<T: AsRef<[u8]>>(params: Params, tubes: &[T]) -> Result<Self, StateError> {
        params.validate()?;
        if tubes.len() != params.n_tubes() {
            return Err(StateError::TubeCount {
                expected: params.n_tubes(),
                got: tubes.len(),
            });
        }
        let mut state = Self::blank(params);
        let mut counts = [0usize; MAX_TUBES];
        for (i, tube) in tubes.iter().enumerate() {
            let tube = tube.as_ref();
            if tube.len() > usize::from(params.capacity) {
                return Err(StateError::TubeOverflow {
                    tube: i,
                    len: tube.len(),
                    capacity: params.capacity,
                });
            }
            for (j, &color) in tube.iter().enumerate() {
                if color >= params.n_colors {
                    return Err(StateError::InvalidColor { tube: i, color });
                }
                counts[usize::from(color)] += 1;
                state.cells[i][j] = color;
            }
            state.heights[i] = to_u8(tube.len());
        }
        for (color, &count) in counts.iter().enumerate().take(params.n_colors.into()) {
            if count != usize::from(params.capacity) {
                return Err(StateError::ColorCount {
                    color: to_u8(color),
                    count,
                    expected: params.capacity,
                });
            }
        }
        Ok(state)
    }

    /// Builds the standard layout: the first `n_colors` tubes are filled bottom to top from
    /// `units` in order, and the last `n_empty` tubes are empty.
    ///
    /// # Errors
    ///
    /// Rejects a wrong number of units and anything [`State::from_tubes`] rejects.
    pub fn from_fill(params: Params, units: &[u8]) -> Result<Self, StateError> {
        params.validate()?;
        if units.len() != params.n_units() {
            return Err(StateError::UnitCount {
                expected: params.n_units(),
                got: units.len(),
            });
        }
        let cap = usize::from(params.capacity);
        let mut tubes: Vec<&[u8]> = units.chunks(cap).collect();
        tubes.resize(params.n_tubes(), &[]);
        Self::from_tubes(params, &tubes)
    }

    /// The solved standard layout: tube `i` is full of color `i`, the last `n_empty` tubes are
    /// empty.
    ///
    /// # Errors
    ///
    /// Rejects invalid params.
    pub fn solved(params: Params) -> Result<Self, StateError> {
        params.validate()?;
        let mut state = Self::blank(params);
        for color in 0..params.n_colors {
            let i = usize::from(color);
            state.cells[i][..usize::from(params.capacity)].fill(color);
            state.heights[i] = params.capacity;
        }
        Ok(state)
    }

    /// The units `0,0,…,1,1,…` (`capacity` of each color) that a standard-layout fill shuffles.
    pub fn sorted_units(params: Params) -> Vec<u8> {
        (0..params.n_colors)
            .flat_map(|c| core::iter::repeat_n(c, params.capacity.into()))
            .collect()
    }

    fn blank(params: Params) -> Self {
        Self {
            params,
            cells: [[EMPTY; MAX_CAP]; MAX_TUBES],
            heights: [0; MAX_TUBES],
        }
    }

    pub const fn params(&self) -> Params {
        self.params
    }

    pub const fn n_tubes(&self) -> usize {
        self.params.n_tubes()
    }

    pub const fn capacity(&self) -> u8 {
        self.params.capacity
    }

    /// Contents of tube `i`, bottom to top. Panics if `i >= MAX_TUBES`.
    pub fn tube(&self, i: usize) -> &[u8] {
        &self.cells[i][..usize::from(self.heights[i])]
    }

    /// Tube `i` padded with [`EMPTY`] up to the capacity.
    pub fn padded_tube(&self, i: usize) -> &[u8] {
        &self.cells[i][..usize::from(self.params.capacity)]
    }

    /// All tubes as owned vectors, bottom to top.
    pub fn tubes(&self) -> Vec<Vec<u8>> {
        (0..self.n_tubes()).map(|i| self.tube(i).to_vec()).collect()
    }

    pub const fn height(&self, i: usize) -> u8 {
        self.heights[i]
    }

    /// Free slots in tube `i`.
    pub const fn free(&self, i: usize) -> u8 {
        self.params.capacity - self.heights[i]
    }

    pub const fn is_tube_empty(&self, i: usize) -> bool {
        self.heights[i] == 0
    }

    pub const fn is_tube_full(&self, i: usize) -> bool {
        self.heights[i] == self.params.capacity
    }

    /// Top color of tube `i`, or `None` if it is empty.
    pub fn top(&self, i: usize) -> Option<u8> {
        let h = usize::from(self.heights[i]);
        (h > 0).then(|| self.cells[i][h - 1])
    }

    /// Length of the maximal same-color run at the top of tube `i` (0 if empty).
    pub fn top_run(&self, i: usize) -> u8 {
        let tube = self.tube(i);
        let Some(&top) = tube.last() else { return 0 };
        to_u8(tube.iter().rev().take_while(|&&c| c == top).count())
    }

    /// Whether every tube is either empty or full and single-colored.
    pub fn is_solved(&self) -> bool {
        (0..self.n_tubes()).all(|i| {
            let tube = self.tube(i);
            tube.is_empty() || (self.is_tube_full(i) && tube.iter().all(|&c| c == tube[0]))
        })
    }

    /// Number of maximal same-color runs over all tubes.
    pub fn segments(&self) -> u32 {
        (0..self.n_tubes())
            .map(|i| {
                let tube = self.tube(i);
                u32::from(!tube.is_empty()) + count_changes(tube)
            })
            .sum()
    }

    /// Number of vertically adjacent cells with different colors, over all tubes.
    pub fn color_changes(&self) -> u32 {
        (0..self.n_tubes())
            .map(|i| count_changes(self.tube(i)))
            .sum()
    }
}

fn count_changes(tube: &[u8]) -> u32 {
    to_u32(tube.windows(2).filter(|w| w[0] != w[1]).count())
}

/// Narrowing for values bounded by `MAX_TUBES` / `MAX_CAP`.
pub(crate) fn to_u8(x: usize) -> u8 {
    u8::try_from(x).expect("value bounded by MAX_TUBES * MAX_CAP")
}

pub(crate) fn to_u32(x: usize) -> u32 {
    u32::try_from(x).expect("value fits in u32")
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "State(c={} cap={} e={}; {self})",
            self.params.n_colors, self.params.capacity, self.params.n_empty
        )
    }
}

impl fmt::Display for State {
    /// Tubes as `[0 1 1] [2] []`, bottom to top.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in 0..self.n_tubes() {
            if i > 0 {
                f.write_str(" ")?;
            }
            f.write_str("[")?;
            for (j, c) in self.tube(i).iter().enumerate() {
                if j > 0 {
                    f.write_str(" ")?;
                }
                write!(f, "{c}")?;
            }
            f.write_str("]")?;
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct StateRepr {
    params: Params,
    tubes: Vec<Vec<u8>>,
}

impl Serialize for State {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        StateRepr {
            params: self.params,
            tubes: self.tubes(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for State {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let repr = StateRepr::deserialize(deserializer)?;
        Self::from_tubes(repr.params, &repr.tubes).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P3: Params = Params {
        n_colors: 3,
        capacity: 3,
        n_empty: 1,
    };

    #[test]
    fn solved_layout() {
        let s = State::solved(P3).unwrap();
        assert!(s.is_solved());
        assert_eq!(
            s.tubes(),
            vec![vec![0, 0, 0], vec![1, 1, 1], vec![2, 2, 2], vec![]]
        );
        assert_eq!(s.segments(), 3);
        assert_eq!(s.color_changes(), 0);
        assert_eq!(s.to_string(), "[0 0 0] [1 1 1] [2 2 2] []");
    }

    #[test]
    fn solved_in_any_tube_order() {
        let s = State::from_tubes(P3, &[&[][..], &[2, 2, 2], &[0, 0, 0], &[1, 1, 1]]).unwrap();
        assert!(s.is_solved());
    }

    #[test]
    fn not_solved() {
        // Mixed tube.
        let s = State::from_fill(P3, &[0, 0, 1, 1, 1, 0, 2, 2, 2]).unwrap();
        assert!(!s.is_solved());
        assert_eq!(s.segments(), 5);
        assert_eq!(s.color_changes(), 2);
        // Single-colored but split over two tubes.
        let s = State::from_tubes(P3, &[&[0, 0][..], &[1, 1, 1], &[2, 2, 2], &[0]]).unwrap();
        assert!(!s.is_solved());
        assert_eq!(s.segments(), 4);
        assert_eq!(s.color_changes(), 0);
    }

    #[test]
    fn accessors() {
        let s = State::from_tubes(P3, &[&[0, 1, 1][..], &[1, 0], &[2, 2, 2], &[0]]).unwrap();
        assert_eq!(s.top(0), Some(1));
        assert_eq!(s.top_run(0), 2);
        assert_eq!(s.top_run(1), 1);
        assert_eq!(s.top_run(2), 3);
        assert_eq!(s.free(1), 1);
        assert!(s.is_tube_full(0));
        assert!(!s.is_tube_empty(3));
        assert_eq!(s.padded_tube(3), &[0, EMPTY, EMPTY]);
    }

    #[test]
    fn validation() {
        assert_eq!(
            State::from_tubes(P3, &[&[0, 0, 0][..], &[1, 1, 1], &[2, 2, 2]]),
            Err(StateError::TubeCount {
                expected: 4,
                got: 3
            })
        );
        assert_eq!(
            State::from_tubes(P3, &[&[0, 0, 0, 1][..], &[1, 1], &[2, 2, 2], &[]]),
            Err(StateError::TubeOverflow {
                tube: 0,
                len: 4,
                capacity: 3
            })
        );
        assert_eq!(
            State::from_tubes(P3, &[&[0, 0, 0][..], &[1, 1, 3], &[2, 2, 2], &[]]),
            Err(StateError::InvalidColor { tube: 1, color: 3 })
        );
        assert_eq!(
            State::from_tubes(P3, &[&[0, 0, 0][..], &[1, 1, 2], &[2, 2, 2], &[]]),
            Err(StateError::ColorCount {
                color: 1,
                count: 2,
                expected: 3
            })
        );
        assert_eq!(
            State::from_fill(P3, &[0, 0, 0]),
            Err(StateError::UnitCount {
                expected: 9,
                got: 3
            })
        );
        let bad = Params { n_colors: 0, ..P3 };
        assert_eq!(
            State::solved(bad),
            Err(StateError::Params(ParamsError::NoColors))
        );
    }

    #[test]
    fn sorted_units_and_fill() {
        let units = State::sorted_units(P3);
        assert_eq!(units, vec![0, 0, 0, 1, 1, 1, 2, 2, 2]);
        assert_eq!(
            State::from_fill(P3, &units).unwrap(),
            State::solved(P3).unwrap()
        );
    }

    #[test]
    fn serde_round_trip() {
        let s = State::from_tubes(P3, &[&[0, 1, 1][..], &[1, 0], &[2, 2, 2], &[0]]).unwrap();
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(
            json,
            r#"{"params":{"n_colors":3,"capacity":3,"n_empty":1},"tubes":[[0,1,1],[1,0],[2,2,2],[0]]}"#
        );
        let back: State = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
        assert!(serde_json::from_str::<State>(&json.replace("[0]]", "[1]]")).is_err());
    }
}
