//! Exhaustive enumeration of standard-layout fills, for tests and small exact analyses (e.g. the
//! uniformity test of Phase 2). The count grows as `(n_colors * capacity)! / capacity!^n_colors`,
//! so this is only usable for tiny params.

use crate::params::{Params, ParamsError};
use crate::state::State;

/// Every distinct standard-layout fill of `params` (the first `n_colors` tubes full, the rest
/// empty), each exactly once, in lexicographic order of the unit sequence.
///
/// # Errors
///
/// Invalid params.
pub fn standard_fills(params: Params) -> Result<StandardFills, ParamsError> {
    params.validate()?;
    Ok(StandardFills {
        params,
        next: Some(State::sorted_units(params)),
    })
}

/// Iterator returned by [`standard_fills`].
#[derive(Clone, Debug)]
pub struct StandardFills {
    params: Params,
    next: Option<Vec<u8>>,
}

impl Iterator for StandardFills {
    type Item = State;

    fn next(&mut self) -> Option<State> {
        let units = self.next.take()?;
        let state = State::from_fill(self.params, &units).expect("a permuted fill is valid");
        let mut following = units;
        if next_permutation(&mut following) {
            self.next = Some(following);
        }
        Some(state)
    }
}

/// Advances `xs` to the next lexicographic permutation (repeated values give each distinct
/// permutation once). Returns `false` and leaves `xs` unchanged at the last one.
fn next_permutation(xs: &mut [u8]) -> bool {
    let Some(i) = (1..xs.len()).rev().find(|&i| xs[i - 1] < xs[i]) else {
        return false;
    };
    let pivot = i - 1;
    let j = (i..xs.len())
        .rev()
        .find(|&j| xs[j] > xs[pivot])
        .expect("xs[i] is larger than the pivot");
    xs.swap(pivot, j);
    xs[i..].reverse();
    true
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
    fn counts_are_multinomial() {
        let count = |params| standard_fills(params).unwrap().count();
        assert_eq!(count(p(1, 3, 1)), 1);
        assert_eq!(count(p(2, 2, 1)), 6);
        assert_eq!(count(p(3, 3, 1)), 1680);
        assert_eq!(count(p(2, 4, 0)), 70);
        assert_eq!(count(p(4, 2, 2)), 2520);
    }

    #[test]
    fn distinct_and_in_layout() {
        let params = p(3, 2, 2);
        let states: Vec<State> = standard_fills(params).unwrap().collect();
        let mut unique = states.clone();
        unique.sort_unstable_by_key(State::tubes);
        unique.dedup();
        assert_eq!(unique.len(), states.len());
        assert_eq!(states.len(), 90);
        assert_eq!(states[0], State::solved(params).unwrap());
        for s in &states {
            assert!((0..3).all(|i| s.is_tube_full(i)));
            assert!((3..5).all(|i| s.is_tube_empty(i)));
        }
        assert!(standard_fills(p(0, 2, 1)).is_err());
    }
}
