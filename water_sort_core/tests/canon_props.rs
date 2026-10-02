//! Property tests for the canonical forms and hash.

mod common;

use common::{below, params_in, permute, random_state, rng};
use proptest::prelude::*;
use water_sort_core::canon::encode;
use water_sort_core::{
    State, canonical_full, canonical_hash, canonical_tubes, is_symmetric, solver_key,
};

fn all_perms(n: usize) -> Vec<Vec<u8>> {
    if n == 0 {
        return vec![Vec::new()];
    }
    let mut out = Vec::new();
    for p in all_perms(n - 1) {
        for pos in 0..=p.len() {
            let mut q = p.clone();
            q.insert(pos, u8::try_from(n - 1).unwrap());
            out.push(q);
        }
    }
    out
}

/// Reference: minimum over all `n!` color relabelings of the sorted-tube encoding.
fn brute_force(s: &State) -> Vec<u8> {
    let identity: Vec<usize> = (0..s.n_tubes()).collect();
    all_perms(usize::from(s.params().n_colors))
        .iter()
        .map(|sigma| encode(&canonical_tubes(&permute(s, &identity, sigma))))
        .min()
        .unwrap()
}

fn random_perm(seed: u64, n: usize) -> Vec<usize> {
    let mut rng = rng(seed);
    let mut p: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        p.swap(i, below(&mut rng, i + 1));
    }
    p
}

fn small_state() -> impl Strategy<Value = State> {
    (params_in(1..=6, 2..=4, 0..=3), any::<u64>()).prop_map(|(p, seed)| random_state(p, seed))
}

fn any_state() -> impl Strategy<Value = State> {
    (params_in(1..=14, 2..=8, 0..=2), any::<u64>())
        .prop_filter("fits", |(p, _)| p.validate().is_ok())
        .prop_map(|(p, seed)| random_state(p, seed))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn canonical_full_matches_brute_force(s in small_state()) {
        prop_assert_eq!(encode(&canonical_full(&s)), brute_force(&s), "{}", s);
    }

    #[test]
    fn canonical_invariant_under_symmetries(s in any_state(), seed in any::<u64>()) {
        let tube_perm = random_perm(seed, s.n_tubes());
        let color_perm: Vec<u8> = random_perm(seed ^ 1, usize::from(s.params().n_colors))
            .into_iter()
            .map(|c| u8::try_from(c).unwrap())
            .collect();
        let t = permute(&s, &tube_perm, &color_perm);
        let c = canonical_full(&s);
        prop_assert_eq!(canonical_full(&t), c);
        prop_assert_eq!(canonical_hash(&t), canonical_hash(&s));
        // The canonical form is a fixed point and a symmetric image of the input.
        prop_assert_eq!(canonical_full(&c), c);
        prop_assert_eq!(c.is_solved(), s.is_solved());
        prop_assert_eq!(c.segments(), s.segments());
    }

    #[test]
    fn tube_order_forms(s in any_state(), seed in any::<u64>()) {
        let identity: Vec<u8> = (0..s.params().n_colors).collect();
        let t = permute(&s, &random_perm(seed, s.n_tubes()), &identity);
        prop_assert_eq!(canonical_tubes(&t), canonical_tubes(&s));
        prop_assert_eq!(solver_key(&t), solver_key(&s));
        // solver_key is sound: it is a symmetric image of the input.
        prop_assert_eq!(canonical_full(&solver_key(&s)), canonical_full(&s));
    }
}

#[test]
fn symmetric_states_match_brute_force() {
    // Highly symmetric layouts stress the tie branching.
    let p = common::params(6, 2, 0);
    let cycle: Vec<Vec<u8>> = (0..6u8).map(|i| vec![i, (i + 1) % 6]).collect();
    let s = State::from_tubes(p, &cycle).unwrap();
    assert_eq!(encode(&canonical_full(&s)), brute_force(&s));
    let p = common::params(6, 4, 0);
    let pairs: Vec<Vec<u8>> = (0..6u8)
        .map(|i| {
            let j = i ^ 1;
            vec![i, j, i, j]
        })
        .collect();
    let s = State::from_tubes(p, &pairs).unwrap();
    assert_eq!(encode(&canonical_full(&s)), brute_force(&s));
}

/// `copies` disjoint copies of a random `base_colors`-color puzzle, plus `n_empty` empty tubes,
/// then shuffled. Such states have large automorphism groups.
fn symmetric_state(base_colors: u8, copies: u8, cap: u8, n_empty: u8, seed: u64) -> State {
    let base = random_state(common::params(base_colors, cap, 0), seed);
    let mut tubes = Vec::new();
    for k in 0..copies {
        for t in base.tubes() {
            tubes.push(t.iter().map(|&c| c + k * base_colors).collect::<Vec<u8>>());
        }
    }
    tubes.extend(core::iter::repeat_n(Vec::new(), n_empty.into()));
    let p = common::params(base_colors * copies, cap, n_empty);
    let s = State::from_tubes(p, &tubes).unwrap();
    let colors: Vec<u8> = random_perm(seed ^ 7, usize::from(p.n_colors))
        .into_iter()
        .map(|c| u8::try_from(c).unwrap())
        .collect();
    permute(&s, &random_perm(seed ^ 3, s.n_tubes()), &colors)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn symmetric_small_match_brute_force(
        (b, m) in prop_oneof![Just((1u8, 6u8)), Just((2, 3)), Just((3, 2)), Just((1, 4)), Just((2, 2))],
        cap in 2u8..=4,
        e in 0u8..=2,
        seed in any::<u64>(),
    ) {
        let s = symmetric_state(b, m, cap, e, seed);
        prop_assert_eq!(encode(&canonical_full(&s)), brute_force(&s), "{}", s);
    }

    #[test]
    fn symmetric_large_invariant(
        (b, m) in prop_oneof![Just((1u8, 12u8)), Just((2, 6)), Just((3, 4)), Just((4, 3)), Just((7, 2))],
        cap in 2u8..=4,
        seed in any::<u64>(),
        seed2 in any::<u64>(),
    ) {
        let s = symmetric_state(b, m, cap, 2, seed);
        let colors: Vec<u8> = random_perm(seed2, usize::from(s.params().n_colors))
            .into_iter()
            .map(|c| u8::try_from(c).unwrap())
            .collect();
        let t = permute(&s, &random_perm(seed2 ^ 1, s.n_tubes()), &colors);
        prop_assert_eq!(canonical_full(&t), canonical_full(&s));
    }
}

/// Reference for [`is_symmetric`]: two identical non-empty tubes, or one of the `n!` color
/// permutations other than the identity maps the sorted non-empty tubes onto themselves.
fn symmetric_brute_force(s: &State) -> bool {
    let sorted_nonempty = |s: &State| {
        let mut tubes: Vec<Vec<u8>> = s.tubes().into_iter().filter(|t| !t.is_empty()).collect();
        tubes.sort();
        tubes
    };
    let tubes = sorted_nonempty(s);
    if tubes.windows(2).any(|w| w[0] == w[1]) {
        return true;
    }
    let identity: Vec<usize> = (0..s.n_tubes()).collect();
    all_perms(usize::from(s.params().n_colors))
        .iter()
        .filter(|sigma| sigma.iter().enumerate().any(|(i, &c)| usize::from(c) != i))
        .any(|sigma| sorted_nonempty(&permute(s, &identity, sigma)) == tubes)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn is_symmetric_matches_brute_force(s in small_state()) {
        prop_assert_eq!(is_symmetric(&s), symmetric_brute_force(&s), "{}", s);
    }

    #[test]
    fn is_symmetric_matches_brute_force_on_fills(
        p in params_in(1..=6, 2..=4, 0..=2),
        seed in any::<u64>(),
    ) {
        let s = common::random_fill(p, seed);
        prop_assert_eq!(is_symmetric(&s), symmetric_brute_force(&s), "{}", s);
    }

    #[test]
    fn is_symmetric_on_symmetric_layouts(
        (b, m) in prop_oneof![Just((1u8, 6u8)), Just((2, 3)), Just((3, 2)), Just((1, 4)), Just((2, 2))],
        cap in 2u8..=4,
        e in 0u8..=2,
        seed in any::<u64>(),
    ) {
        let s = symmetric_state(b, m, cap, e, seed);
        prop_assert!(is_symmetric(&s), "{}", s);
        prop_assert!(symmetric_brute_force(&s), "{}", s);
    }

    #[test]
    fn is_symmetric_is_invariant(s in small_state(), seed in any::<u64>()) {
        let n = usize::from(s.params().n_colors);
        let colors: Vec<u8> = random_perm(seed, n).into_iter().map(|c| u8::try_from(c).unwrap()).collect();
        let t = permute(&s, &random_perm(seed ^ 1, s.n_tubes()), &colors);
        prop_assert_eq!(is_symmetric(&t), is_symmetric(&s));
    }
}

#[test]
fn is_symmetric_exhaustive_small() {
    // Every 2-color cap-3 and 3-color cap-2 fill against the brute force.
    for p in [
        common::params(2, 3, 1),
        common::params(3, 2, 1),
        common::params(3, 3, 0),
    ] {
        let (mut yes, mut total) = (0, 0);
        for s in water_sort_core::standard_fills(p).unwrap() {
            assert_eq!(is_symmetric(&s), symmetric_brute_force(&s), "{s}");
            yes += usize::from(is_symmetric(&s));
            total += 1;
        }
        assert!(yes > 0 && yes < total);
    }
}

#[test]
fn is_symmetric_on_large_puzzles() {
    // 12 colors: most random fills have no symmetry; doubled layouts always do.
    let p = common::params(12, 4, 2);
    let asymmetric = (0..200)
        .filter(|&seed| !is_symmetric(&common::random_fill(p, seed)))
        .count();
    assert!(asymmetric > 190, "{asymmetric}");
    for seed in 0..20 {
        assert!(is_symmetric(&symmetric_state(6, 2, 4, 2, seed)));
        assert!(is_symmetric(&symmetric_state(3, 4, 4, 2, seed)));
    }
}
