//! Canonical forms under the game's symmetries (tube order and color names) and the stable
//! canonical hash.
//!
//! See D8: "sort the tubes, then relabel colors by first appearance" is not a canonical form,
//! because relabeling changes the sort order. [`canonical_full`] is exact; [`solver_key`] is the
//! cheap approximation the solver uses.

use crate::params::{MAX_CAP, MAX_TUBES};
use crate::state::{EMPTY, State};

type Tube = [u8; MAX_CAP];

fn raw_tubes(s: &State) -> Vec<Tube> {
    (0..s.n_tubes()).map(|i| s.raw_tube(i)).collect()
}

/// Relabels colors in order of first appearance, scanning tubes in order, bottom to top.
fn relabel_first_appearance(tubes: &mut [Tube]) {
    let mut map = [EMPTY; MAX_TUBES];
    let mut next = 0u8;
    for tube in tubes.iter_mut() {
        for cell in tube.iter_mut().take_while(|c| **c != EMPTY) {
            let slot = &mut map[usize::from(*cell)];
            if *slot == EMPTY {
                *slot = next;
                next += 1;
            }
            *cell = *slot;
        }
    }
}

/// Tubes sorted lexicographically (bottom to top, [`EMPTY`]-padded, so empty tubes sort last).
/// Canonical under tube order only.
pub fn canonical_tubes(s: &State) -> State {
    let mut tubes = raw_tubes(s);
    tubes.sort_unstable();
    State::from_raw_tubes(s.params(), &tubes)
}

/// Sort, relabel colors by first appearance, sort again.
///
/// Fast but not exact: two symmetric states can get different keys. It is sound for the solver:
/// the key is always a symmetric image of `s`, so equal keys imply equivalent states, and a missed
/// merge only costs speed.
pub fn solver_key(s: &State) -> State {
    let mut tubes = raw_tubes(s);
    tubes.sort_unstable();
    relabel_first_appearance(&mut tubes);
    tubes.sort_unstable();
    State::from_raw_tubes(s.params(), &tubes)
}

/// The exact canonical form: the lexicographic minimum of the sorted-tube encoding over all
/// color relabelings (D8).
///
/// For a fixed relabeling, sorting the tubes gives the minimal concatenation over all tube
/// orders, and for a fixed tube order, first-appearance labeling gives the minimal relabeling.
/// The minimum over relabelings is therefore the minimum over tube orders of the first-appearance
/// labeled concatenation. That is built tube by tube: each position takes the smallest block
/// any remaining tube can produce under the labels fixed so far, branching only over tubes with
/// distinct contents that tie. Once every color has a label the rest is a plain sort.
///
/// Symmetric puzzles make the ties branch a lot, so the search also prunes by automorphisms
/// (as in nauty): two complete orders with equal encodings reveal a color permutation that maps
/// the puzzle onto itself. At a branch point, a candidate tube that such permutations (fixing
/// every already-labeled color) map onto an explored candidate leads to an identical subtree and
/// is skipped.
///
/// # Panics
///
/// Never for a valid [`State`]: the search always completes at least one tube order.
pub fn canonical_full(s: &State) -> State {
    let tubes = raw_tubes(s);
    let mut search = Search {
        tubes: &tubes,
        n_colors: s.params().n_colors,
        used: vec![false; tubes.len()],
        prefix: Vec::with_capacity(tubes.len()),
        best: None,
        best_map: [EMPTY; MAX_TUBES],
        automorphisms: Vec::new(),
    };
    search.run(&[EMPTY; MAX_TUBES], 0);
    let best = search
        .best
        .expect("search visits at least one complete order");
    State::from_raw_tubes(s.params(), &best)
}

/// Color map `color -> label` (or `color -> color` for automorphisms); [`EMPTY`] = unassigned.
type ColorMap = [u8; MAX_TUBES];

/// Upper bound on stored automorphisms; more only cost time, never correctness.
const MAX_AUTOMORPHISMS: usize = 64;

struct Search<'a> {
    tubes: &'a [Tube],
    n_colors: u8,
    used: Vec<bool>,
    prefix: Vec<Tube>,
    best: Option<Vec<Tube>>,
    /// Labeling that produced `best`.
    best_map: ColorMap,
    automorphisms: Vec<ColorMap>,
}

/// Relabels `tube` under `map`, giving unlabeled colors the next free labels in order of
/// appearance. Returns the block and the extended map.
fn label_tube(tube: Tube, map: &ColorMap, next: u8) -> (Tube, ColorMap, u8) {
    let mut map = *map;
    let mut next = next;
    let mut block = tube;
    for cell in block.iter_mut().take_while(|c| **c != EMPTY) {
        let slot = &mut map[usize::from(*cell)];
        if *slot == EMPTY {
            *slot = next;
            next += 1;
        }
        *cell = *slot;
    }
    (block, map, next)
}

fn map_tube(tube: Tube, gamma: &ColorMap) -> Tube {
    let mut out = tube;
    for cell in out.iter_mut().take_while(|c| **c != EMPTY) {
        *cell = gamma[usize::from(*cell)];
    }
    out
}

impl Search<'_> {
    /// Compares the current prefix (plus `block` at its end) against the best complete order.
    fn cmp_with_best(&self, block: Tube) -> core::cmp::Ordering {
        let Some(best) = &self.best else {
            return core::cmp::Ordering::Less;
        };
        let depth = self.prefix.len();
        self.prefix
            .iter()
            .chain(core::iter::once(&block))
            .cmp(best[..=depth].iter())
    }

    /// Whether `tube` is in the orbit of the `explored` contents under the known automorphisms
    /// that fix every color labeled in `map`.
    fn in_explored_orbit(&self, tube: Tube, explored: &[Tube], map: &ColorMap) -> bool {
        let gens: Vec<&ColorMap> = self
            .automorphisms
            .iter()
            .filter(|g| {
                (0..self.n_colors).all(|c| map[usize::from(c)] == EMPTY || g[usize::from(c)] == c)
            })
            .collect();
        if gens.is_empty() {
            return false;
        }
        let mut orbit: Vec<Tube> = explored.to_vec();
        let mut k = 0;
        while k < orbit.len() {
            for g in &gens {
                let image = map_tube(orbit[k], g);
                if !orbit.contains(&image) {
                    orbit.push(image);
                }
            }
            k += 1;
        }
        orbit.contains(&tube)
    }

    fn run(&mut self, map: &ColorMap, next: u8) {
        if next == self.n_colors {
            self.finish(map);
            return;
        }
        // Smallest block any remaining tube can produce next.
        let mut min_block: Option<Tube> = None;
        for (i, tube) in self.tubes.iter().enumerate() {
            if !self.used[i] {
                let (block, _, _) = label_tube(*tube, map, next);
                if min_block.is_none_or(|m| block < m) {
                    min_block = Some(block);
                }
            }
        }
        let Some(min_block) = min_block else {
            unreachable!("every color is in some tube, so tubes remain while colors are unlabeled")
        };
        if self.cmp_with_best(min_block).is_gt() {
            return;
        }
        // Branch over remaining tubes with distinct contents that produce the minimal block.
        let mut explored: Vec<Tube> = Vec::new();
        for i in 0..self.tubes.len() {
            let tube = self.tubes[i];
            if self.used[i] || (0..i).any(|j| !self.used[j] && self.tubes[j] == tube) {
                continue;
            }
            let (block, next_map, next_label) = label_tube(tube, map, next);
            if block != min_block || self.in_explored_orbit(tube, &explored, map) {
                continue;
            }
            self.used[i] = true;
            self.prefix.push(block);
            self.run(&next_map, next_label);
            self.prefix.pop();
            self.used[i] = false;
            explored.push(tube);
        }
    }

    /// All colors are labeled: the rest of the order is the sorted relabeled remainder.
    fn finish(&mut self, map: &ColorMap) {
        let mut rest: Vec<Tube> = (0..self.tubes.len())
            .filter(|&i| !self.used[i])
            .map(|i| label_tube(self.tubes[i], map, self.n_colors).0)
            .collect();
        rest.sort_unstable();
        let mut candidate = self.prefix.clone();
        candidate.extend(rest);
        match self.best.as_ref().map(|b| candidate.cmp(b)) {
            None | Some(core::cmp::Ordering::Less) => {
                self.best = Some(candidate);
                self.best_map = *map;
            }
            Some(core::cmp::Ordering::Equal) => {
                // Both labelings give the same multiset of tubes, so
                // gamma = best_map^-1 . map maps the puzzle onto itself.
                if self.automorphisms.len() < MAX_AUTOMORPHISMS {
                    let mut inverse = [EMPTY; MAX_TUBES];
                    for c in 0..self.n_colors {
                        inverse[usize::from(self.best_map[usize::from(c)])] = c;
                    }
                    let mut gamma = [EMPTY; MAX_TUBES];
                    for c in 0..self.n_colors {
                        gamma[usize::from(c)] = inverse[usize::from(map[usize::from(c)])];
                    }
                    self.automorphisms.push(gamma);
                }
            }
            Some(core::cmp::Ordering::Greater) => {}
        }
    }
}

/// Fixed byte encoding: `[n_colors, capacity, n_empty]`, then each tube's `capacity` cells
/// bottom to top, with [`EMPTY`] (`0xFF`) above the fill height. Platform independent.
pub fn encode(s: &State) -> Vec<u8> {
    let p = s.params();
    let mut out = Vec::with_capacity(3 + p.n_tubes() * usize::from(p.capacity));
    out.extend([p.n_colors, p.capacity, p.n_empty]);
    for i in 0..s.n_tubes() {
        out.extend_from_slice(s.padded_tube(i));
    }
    out
}

/// `xxh3_64(encode(canonical_full(s)))`: identical for all states that differ only by tube order
/// and color names, stable across platforms and Rust releases. Used for dedup and splits.
pub fn canonical_hash(s: &State) -> u64 {
    xxhash_rust::xxh3::xxh3_64(&encode(&canonical_full(s)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Params;

    const P: Params = Params {
        n_colors: 3,
        capacity: 3,
        n_empty: 1,
    };

    fn st(tubes: &[&[u8]]) -> State {
        State::from_tubes(P, tubes).unwrap()
    }

    #[test]
    fn sort_then_relabel_is_not_canonical() {
        // Two color-permuted copies of one puzzle. Sort-then-relabel maps them to different
        // encodings; canonical_full does not.
        let a = st(&[&[0, 1, 2], &[1, 0, 0], &[2, 2, 1], &[]]);
        let b = st(&[&[2, 0, 1], &[0, 2, 2], &[1, 1, 0], &[]]); // 0->2, 1->0, 2->1
        let sort_relabel = |s: &State| {
            let mut t = raw_tubes(s);
            t.sort_unstable();
            relabel_first_appearance(&mut t);
            t
        };
        assert_ne!(sort_relabel(&a), sort_relabel(&b));
        assert_eq!(canonical_full(&a), canonical_full(&b));
        assert_eq!(canonical_hash(&a), canonical_hash(&b));
    }

    #[test]
    fn canonical_full_example() {
        let s = st(&[&[2, 2, 1], &[], &[1, 0, 0], &[0, 1, 2]]);
        let c = canonical_full(&s);
        assert_eq!(
            c.tubes(),
            vec![vec![0, 0, 1], vec![1, 2, 2], vec![2, 1, 0], vec![]]
        );
        assert_eq!(canonical_tubes(&s).tube(3), &[] as &[u8]);
        assert_eq!(encode(&c)[..6], [3, 3, 1, 0, 0, 1]);
        assert_eq!(encode(&c).len(), 3 + 4 * 3);
    }

    #[test]
    fn hash_separates_params() {
        let a = State::solved(Params {
            n_colors: 2,
            capacity: 2,
            n_empty: 1,
        })
        .unwrap();
        let b = State::solved(Params {
            n_colors: 2,
            capacity: 2,
            n_empty: 2,
        })
        .unwrap();
        assert_ne!(canonical_hash(&a), canonical_hash(&b));
    }
}
