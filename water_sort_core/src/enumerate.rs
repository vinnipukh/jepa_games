//! Exhaustive enumeration of standard-layout and distributed-layout fills, for tests and small
//! exact analyses (e.g. the uniformity tests). The standard count grows as
//! `(n_colors * capacity)! / capacity!^n_colors`, and the distributed count is that times the
//! number of height vectors, so this is only usable for tiny params.

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

/// Every distinct distributed-layout state of `params` (D14), each exactly once: for every valid
/// height vector in lexicographic order, every distinct unit sequence in lexicographic order,
/// packed with [`State::from_heights`].
///
/// # Errors
///
/// Invalid params.
pub fn distributed_fills(params: Params) -> Result<DistributedFills, ParamsError> {
    params.validate()?;
    let mut heights = vec![0u8; params.n_tubes()];
    let found = first_heights(&mut heights, params);
    debug_assert!(found, "a valid params value has at least one height vector");
    Ok(DistributedFills {
        params,
        heights,
        next: Some(State::sorted_units(params)),
    })
}

/// Iterator returned by [`distributed_fills`].
#[derive(Clone, Debug)]
pub struct DistributedFills {
    params: Params,
    heights: Vec<u8>,
    next: Option<Vec<u8>>,
}

impl Iterator for DistributedFills {
    type Item = State;

    fn next(&mut self) -> Option<State> {
        let units = self.next.take()?;
        let state = State::from_heights(self.params, &self.heights, &units)
            .expect("a valid height vector with permuted units is a valid state");
        let mut following = units;
        if next_permutation(&mut following) {
            self.next = Some(following);
        } else if next_heights(&mut self.heights, self.params) {
            following.sort_unstable();
            self.next = Some(following);
        }
        Some(state)
    }
}

/// Sets `h` to the lexicographically smallest valid height vector.
fn first_heights(h: &mut [u8], params: Params) -> bool {
    fill_smallest(h, params.n_units(), params.capacity)
}

/// Fills `h` with the lexicographically smallest vector in `[0, cap]^len` summing to `total`:
/// as much as possible goes to the last positions.
fn fill_smallest(h: &mut [u8], mut total: usize, cap: u8) -> bool {
    for x in h.iter_mut().rev() {
        let v = total.min(usize::from(cap));
        *x = u8::try_from(v).expect("bounded by cap");
        total -= v;
    }
    total == 0
}

/// Advances `h` to the next valid height vector in lexicographic order.
fn next_heights(h: &mut [u8], params: Params) -> bool {
    let cap = params.capacity;
    // Increase the rightmost position that can grow while the suffix after it can still give
    // up one unit, then refill that suffix as small as possible.
    for i in (0..h.len().saturating_sub(1)).rev() {
        let suffix: usize = h[i + 1..].iter().map(|&x| usize::from(x)).sum();
        if h[i] < cap && suffix > 0 {
            h[i] += 1;
            return fill_smallest(&mut h[i + 1..], suffix - 1, cap);
        }
    }
    false
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

    #[test]
    fn distributed_counts() {
        let count = |params| distributed_fills(params).unwrap().count();
        // Height vectors x multinomial: 6 x 6, 20 x 1680.
        assert_eq!(count(p(2, 2, 1)), 36);
        assert_eq!(count(p(3, 3, 1)), 20 * 1680);
        assert_eq!(count(p(1, 3, 1)), 4);
        assert_eq!(count(p(2, 4, 0)), 70);
        let n_vectors = |params| crate::layout::HeightCounts::new(params).total();
        for params in [p(2, 3, 2), p(3, 2, 2)] {
            let multinomial = standard_fills(params).unwrap().count() as u64;
            assert_eq!(count(params) as u64, n_vectors(params) * multinomial);
        }
    }

    #[test]
    fn distributed_distinct_and_superset() {
        let params = p(2, 3, 1);
        let states: Vec<State> = distributed_fills(params).unwrap().collect();
        let mut unique = states.clone();
        unique.sort_unstable_by_key(State::tubes);
        unique.dedup();
        assert_eq!(unique.len(), states.len());
        for s in standard_fills(params).unwrap() {
            assert!(states.contains(&s));
        }
        assert!(distributed_fills(p(0, 2, 1)).is_err());
    }
}
