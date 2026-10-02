//! Exact symmetry detection, for the D2 measurement of how far labeled-uniform sampling is from
//! canonical-uniform.
//!
//! A labeled state's symmetry class (all tube reorderings and color renamings of it) has the
//! maximal size exactly when no nontrivial symmetry maps the state onto itself. Within a layout,
//! the empty tubes are interchangeable in every state alike, so they are ignored here.

use crate::params::{MAX_CAP, MAX_TUBES};
use crate::state::{EMPTY, State};

type Tube = [u8; MAX_CAP];

/// Whether `s` has a nontrivial symmetry: two identical non-empty tubes, or a color permutation
/// other than the identity that maps the multiset of tubes onto itself. Such a state's class is
/// smaller than maximal, so labeled-uniform sampling under-samples it (D2).
///
/// Exact. Colors are first split by color refinement (a color can only map to a color with the
/// same refined signature); the remaining choices are searched with backtracking that checks
/// every tube as soon as all its colors are mapped.
pub fn is_symmetric(s: &State) -> bool {
    let mut tubes: Vec<Tube> = (0..s.n_tubes())
        .filter(|&i| !s.is_tube_empty(i))
        .map(|i| s.raw_tube(i))
        .collect();
    tubes.sort_unstable();
    if tubes.windows(2).any(|w| w[0] == w[1]) {
        return true;
    }
    let n = usize::from(s.params().n_colors);
    let class = refine(&tubes, n);
    let mut sorted = class.clone();
    sorted.sort_unstable();
    sorted.dedup();
    if sorted.len() == n {
        // Every color is distinguishable: only the identity is left.
        return false;
    }
    let mut search = PermSearch {
        tubes: &tubes,
        class: &class,
        // Tubes become checkable once their largest color is mapped.
        ready: ready_tubes(&tubes, n),
        image: [EMPTY; MAX_TUBES],
        taken: [false; MAX_TUBES],
        matched: vec![false; tubes.len()],
    };
    search.run(0, n, true)
}

/// Colors in `tube`, bottom to top.
fn cells(tube: &Tube) -> impl Iterator<Item = u8> + '_ {
    tube.iter().copied().take_while(|&c| c != EMPTY)
}

/// A color's class and its occurrences: `(position mask, classes of the tube's cells)` per tube.
type Signature = (u32, Vec<(u8, Vec<u32>)>);

/// Color refinement: starts from one class and splits colors by the multiset of
/// `(positions in the tube, classes of the tube's cells)` over the tubes they occur in, until
/// stable. Symmetric colors always end in the same class.
fn refine(tubes: &[Tube], n: usize) -> Vec<u32> {
    let mut class = vec![0u32; n];
    let mut n_classes = 1;
    loop {
        let signatures: Vec<Signature> = (0..n)
            .map(|c| {
                let mut occurrences: Vec<(u8, Vec<u32>)> = tubes
                    .iter()
                    .filter_map(|t| {
                        let mask = cells(t)
                            .enumerate()
                            .filter(|&(_, x)| usize::from(x) == c)
                            .fold(0u8, |m, (i, _)| m | (1 << i));
                        (mask != 0)
                            .then(|| (mask, cells(t).map(|x| class[usize::from(x)]).collect()))
                    })
                    .collect();
                occurrences.sort_unstable();
                (class[c], occurrences)
            })
            .collect();
        let mut distinct = signatures.clone();
        distinct.sort_unstable();
        distinct.dedup();
        let next: Vec<u32> = signatures
            .iter()
            .map(|sig| {
                let idx = distinct.binary_search(sig).expect("signature is listed");
                u32::try_from(idx).expect("class count fits in u32")
            })
            .collect();
        if distinct.len() == n_classes {
            return next;
        }
        n_classes = distinct.len();
        class = next;
    }
}

/// `ready[c]` lists the tubes whose largest color is `c`.
fn ready_tubes(tubes: &[Tube], n: usize) -> Vec<Vec<usize>> {
    let mut ready = vec![Vec::new(); n];
    for (i, t) in tubes.iter().enumerate() {
        let max = cells(t).max().expect("tubes are non-empty");
        ready[usize::from(max)].push(i);
    }
    ready
}

struct PermSearch<'a> {
    /// Sorted, pairwise distinct, non-empty.
    tubes: &'a [Tube],
    class: &'a [u32],
    ready: Vec<Vec<usize>>,
    /// `image[c]`: where color `c` maps.
    image: [u8; MAX_TUBES],
    taken: [bool; MAX_TUBES],
    /// Tubes already used as the image of some tube.
    matched: Vec<bool>,
}

impl PermSearch<'_> {
    /// Maps colors `c..n`; `identity` says whether colors `0..c` all map to themselves. Returns
    /// whether a non-identity automorphism exists.
    fn run(&mut self, c: usize, n: usize, identity: bool) -> bool {
        if c == n {
            return !identity;
        }
        for d in 0..n {
            if self.taken[d] || self.class[d] != self.class[c] {
                continue;
            }
            self.image[c] = u8::try_from(d).expect("color fits in u8");
            self.taken[d] = true;
            let mut used = Vec::new();
            let consistent = self.match_ready(c, &mut used);
            if consistent && self.run(c + 1, n, identity && c == d) {
                return true;
            }
            for i in used {
                self.matched[i] = false;
            }
            self.taken[d] = false;
        }
        false
    }

    /// Maps every tube that just became fully mapped and claims its image among the tubes.
    /// Tubes are distinct, so each image must be a distinct, still unclaimed tube.
    fn match_ready(&mut self, c: usize, used: &mut Vec<usize>) -> bool {
        for k in 0..self.ready[c].len() {
            let t = self.tubes[self.ready[c][k]];
            let mut mapped = t;
            for cell in mapped.iter_mut().take_while(|x| **x != EMPTY) {
                *cell = self.image[usize::from(*cell)];
            }
            match self.tubes.binary_search(&mapped) {
                Ok(j) if !self.matched[j] => {
                    self.matched[j] = true;
                    used.push(j);
                }
                _ => return false,
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Params;

    const fn p(n_colors: u8, capacity: u8, n_empty: u8) -> Params {
        Params {
            n_colors,
            capacity,
            n_empty,
        }
    }

    #[test]
    fn examples() {
        // Solved: every color permutation is a symmetry.
        assert!(is_symmetric(&State::solved(p(3, 3, 1)).unwrap()));
        // Two identical tubes.
        let s = State::from_tubes(p(2, 2, 1), &[&[0, 1][..], &[0, 1], &[]]).unwrap();
        assert!(is_symmetric(&s));
        // Swapping colors 0 and 1 maps [0,1],[1,0] onto itself.
        let s = State::from_tubes(p(2, 2, 1), &[&[0, 1][..], &[1, 0], &[]]).unwrap();
        assert!(is_symmetric(&s));
        // A 3-cycle of colors: [0,1],[1,2],[2,0].
        let s = State::from_tubes(p(3, 2, 0), &[&[0, 1][..], &[1, 2], &[2, 0]]).unwrap();
        assert!(is_symmetric(&s));
        // No symmetry.
        let s =
            State::from_tubes(p(3, 3, 1), &[&[0, 0, 1][..], &[1, 2, 2], &[2, 1, 0], &[]]).unwrap();
        assert!(!is_symmetric(&s));
        // Empty tubes alone do not count.
        let s = State::from_tubes(p(2, 2, 2), &[&[0, 0][..], &[1], &[1], &[]]).unwrap();
        assert!(is_symmetric(&s));
        let s = State::from_tubes(p(2, 3, 2), &[&[0, 0, 1][..], &[1, 1, 0], &[], &[]]).unwrap();
        assert!(is_symmetric(&s));
        let s = State::from_tubes(p(2, 3, 2), &[&[0, 0, 1][..], &[0, 1, 1], &[], &[]]).unwrap();
        assert!(!is_symmetric(&s));
    }
}
