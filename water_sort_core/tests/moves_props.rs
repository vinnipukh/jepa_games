//! Property tests for forward and reverse moves.

mod common;

use common::{any_state, color_counts};
use proptest::prelude::*;
use water_sort_core::moves::{ReverseMove, reverse_moves, unapply};
use water_sort_core::{Move, State, apply, is_legal, legal_moves};

/// Raw transfer of `count` top units, with no rule checks. `None` if physically impossible.
fn raw_transfer(s: &State, from: usize, to: usize, count: usize) -> Option<State> {
    let mut tubes = s.tubes();
    if from == to || count == 0 || count > usize::from(s.top_run(from)) {
        return None;
    }
    let at = tubes[from].len() - count;
    let moved = tubes[from].split_off(at);
    tubes[to].extend(moved);
    State::from_tubes(s.params(), &tubes).ok()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn forward_moves_preserve_units(s in any_state()) {
        let before = color_counts(&s);
        for m in legal_moves(&s) {
            let (next, k) = apply(&s, m).unwrap();
            prop_assert!(k >= 1);
            prop_assert_eq!(color_counts(&next), before.clone());
        }
    }

    #[test]
    fn reverse_moves_preserve_units(s in any_state()) {
        let before = color_counts(&s);
        for r in reverse_moves(&s) {
            prop_assert_eq!(color_counts(&unapply(&s, r).unwrap()), before.clone());
        }
    }

    /// `apply(unapply(s, r), r.forward()) == (s, r.count)` for every listed reverse move, and
    /// every raw un-pour with that property is listed (the conditions are exact).
    #[test]
    fn reverse_round_trip_is_exact(s in any_state()) {
        let listed = reverse_moves(&s);
        let n = s.n_tubes();
        for from in 0..n {
            for to in 0..n {
                for count in 1..=usize::from(s.capacity()) {
                    let r = ReverseMove::new(
                        u8::try_from(from).unwrap(),
                        u8::try_from(to).unwrap(),
                        u8::try_from(count).unwrap(),
                    );
                    let round_trips = raw_transfer(&s, from, to, count).is_some_and(|prev| {
                        apply(&prev, Move::new(r.to, r.from)) == Ok((s, r.count))
                    });
                    prop_assert_eq!(listed.contains(&r), round_trips, "{:?} in {}", r, s);
                    if round_trips {
                        prop_assert_eq!(unapply(&s, r), Ok(raw_transfer(&s, from, to, count).unwrap()));
                    } else {
                        prop_assert!(unapply(&s, r).is_err());
                    }
                }
            }
        }
    }

    /// Every legal forward pour is undone by some reverse move.
    #[test]
    fn every_pour_has_a_reverse(s in any_state()) {
        for m in legal_moves(&s) {
            let (next, k) = apply(&s, m).unwrap();
            let r = ReverseMove::new(m.to, m.from, k);
            prop_assert!(reverse_moves(&next).contains(&r), "{} after {} in {}", r.count, m, s);
            prop_assert_eq!(unapply(&next, r), Ok(s));
        }
    }

    #[test]
    fn mask_matches_legality(s in any_state()) {
        let mask = water_sort_core::action_mask(&s);
        let n = s.n_tubes();
        for (i, &legal) in mask.iter().enumerate() {
            let m = Move::from_action_index(i, n).unwrap();
            prop_assert_eq!(legal, is_legal(&s, m));
        }
    }
}
