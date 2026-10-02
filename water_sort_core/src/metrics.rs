//! Difficulty metrics recorded with every generated puzzle.

use rand_core::{Rng, SeedableRng};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};

use crate::canon::{canonical_full, canonical_hash};
use crate::generator::{Accepted, GenConfig, MetricsConfig};
use crate::moves::{Move, apply, legal_moves};
use crate::sampling::bounded_u32;
use crate::solver::{SolveResult, SolverLimits, solve};
use crate::state::State;

/// Difficulty measurements of one puzzle.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DifficultyMetrics {
    pub opt_moves: u32,
    /// States the A* solver expanded.
    pub states_expanded: u64,
    pub color_changes: u32,
    pub segments: u32,
    pub random_rollouts: u32,
    /// Fraction of rollouts that reached a state with no legal moves.
    pub random_stuck_rate: f32,
    /// Fraction of rollouts that hit the step cap without solving or getting stuck.
    pub random_capped_rate: f32,
    /// Fraction of distinct depth-1 states that are unsolvable (opt-in).
    pub dead_end_ratio_d1: Option<f32>,
    /// Fraction of distinct depth-2 states that are unsolvable (opt-in).
    pub dead_end_ratio_d2: Option<f32>,
}

/// Computes the metrics for an accepted puzzle.
///
/// Rollouts and dead-end ratios run on [`canonical_full`]`(state)`, so they depend only on the
/// puzzle's symmetry class. The rollout RNG is `R::seed_from_u64(canonical_hash)`, which makes
/// the metrics deterministic for every generator; both generators pass `ChaCha20Rng`.
pub fn compute_metrics<R: Rng + SeedableRng>(
    state: &State,
    accepted: &Accepted,
    cfg: &GenConfig,
) -> DifficultyMetrics {
    let canonical = canonical_full(state);
    let mc = &cfg.metrics;
    let (stuck, capped) = rollouts::<R>(&canonical, accepted.opt_moves, canonical_hash(state), mc);
    let rate = |count: u32| {
        if mc.random_rollouts == 0 {
            0.0
        } else {
            ratio(count, mc.random_rollouts)
        }
    };
    let (d1, d2) = if mc.dead_end_ratios {
        dead_end_ratios(&canonical, &cfg.solver_limits())
    } else {
        (None, None)
    };
    DifficultyMetrics {
        opt_moves: accepted.opt_moves,
        states_expanded: accepted.states_expanded,
        color_changes: state.color_changes(),
        segments: state.segments(),
        random_rollouts: mc.random_rollouts,
        random_stuck_rate: rate(stuck),
        random_capped_rate: rate(capped),
        dead_end_ratio_d1: d1,
        dead_end_ratio_d2: d2,
    }
}

#[allow(clippy::cast_precision_loss)] // counts are far below 2^24
fn ratio(num: u32, den: u32) -> f32 {
    num as f32 / den as f32
}

/// Random legal-move rollouts. Returns the `(stuck, capped)` counts; the rest solved the puzzle.
fn rollouts<R: Rng + SeedableRng>(
    start: &State,
    opt_moves: u32,
    seed: u64,
    mc: &MetricsConfig,
) -> (u32, u32) {
    let mut rng = R::seed_from_u64(seed);
    let cap = mc.rollout_cap_factor.saturating_mul(opt_moves).max(1);
    let (mut stuck, mut capped) = (0, 0);
    let mut moves: Vec<Move> = Vec::new();
    for _ in 0..mc.random_rollouts {
        let mut cur = *start;
        let mut steps = 0;
        while !cur.is_solved() {
            moves.clear();
            moves.extend(legal_moves(&cur));
            if moves.is_empty() {
                stuck += 1;
                break;
            }
            if steps == cap {
                capped += 1;
                break;
            }
            let n = u32::try_from(moves.len()).expect("move count fits in u32");
            let m = moves[bounded_u32(&mut rng, n) as usize];
            cur = apply(&cur, m).expect("listed moves are legal").0;
            steps += 1;
        }
    }
    (stuck, capped)
}

/// Distinct (up to symmetry) children of `states` not in `seen`; adds them to `seen`.
fn expand(states: &[State], seen: &mut FxHashSet<State>) -> Vec<State> {
    let mut out = Vec::new();
    for s in states {
        for m in legal_moves(s) {
            let child = canonical_full(&apply(s, m).expect("listed moves are legal").0);
            if seen.insert(child) {
                out.push(child);
            }
        }
    }
    out
}

/// Fraction of `states` that are unsolvable. States where the solver hits its limit are left
/// out; `None` if no state was decided.
fn unsolvable_ratio(states: &[State], limits: &SolverLimits) -> Option<f32> {
    let (mut bad, mut decided) = (0, 0);
    for s in states {
        match solve(s, limits) {
            SolveResult::Solvable { .. } => decided += 1,
            SolveResult::Unsolvable { .. } => {
                bad += 1;
                decided += 1;
            }
            SolveResult::Timeout { .. } => {}
        }
    }
    (decided > 0).then(|| ratio(bad, decided))
}

/// Unsolvable fractions of the distinct depth-1 and depth-2 states.
fn dead_end_ratios(start: &State, limits: &SolverLimits) -> (Option<f32>, Option<f32>) {
    let mut seen: FxHashSet<State> = FxHashSet::default();
    seen.insert(canonical_full(start));
    let depth1 = expand(&[*start], &mut seen);
    let depth2 = expand(&depth1, &mut seen);
    (
        unsolvable_ratio(&depth1, limits),
        unsolvable_ratio(&depth2, limits),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::evaluate;
    use crate::params::Params;
    use rand_chacha::ChaCha20Rng;

    const P: Params = Params {
        n_colors: 4,
        capacity: 4,
        n_empty: 1,
    };

    fn puzzle() -> State {
        State::from_fill(P, &[0, 1, 2, 3, 3, 2, 1, 0, 1, 0, 3, 2, 2, 3, 0, 1]).unwrap()
    }

    fn with_dead_ends() -> GenConfig {
        GenConfig {
            metrics: MetricsConfig {
                dead_end_ratios: true,
                ..MetricsConfig::default()
            },
            ..GenConfig::default()
        }
    }

    #[test]
    fn deterministic_and_symmetry_invariant() {
        let s = puzzle();
        let cfg = with_dead_ends();
        let accepted = evaluate(&s, &cfg).unwrap();
        let m = compute_metrics::<ChaCha20Rng>(&s, &accepted, &cfg);
        assert_eq!(m, compute_metrics::<ChaCha20Rng>(&s, &accepted, &cfg));
        assert_eq!(m.opt_moves, accepted.opt_moves);
        assert_eq!(m.segments, 16);
        assert_eq!(m.color_changes, 12);
        assert!(m.random_stuck_rate + m.random_capped_rate <= 1.0);
        assert!(m.dead_end_ratio_d1.is_some() && m.dead_end_ratio_d2.is_some());

        // Swap colors 0 and 1 and reverse the tube order: same class, same metrics
        // (states_expanded comes from the labeled solve and may differ).
        let mut tubes: Vec<Vec<u8>> = s
            .tubes()
            .into_iter()
            .map(|t| {
                t.into_iter()
                    .map(|c| [1, 0, 2, 3][usize::from(c)])
                    .collect()
            })
            .collect();
        tubes.reverse();
        let t = State::from_tubes(P, &tubes).unwrap();
        let mt = compute_metrics::<ChaCha20Rng>(&t, &evaluate(&t, &cfg).unwrap(), &cfg);
        assert_eq!(
            DifficultyMetrics {
                states_expanded: m.states_expanded,
                ..mt
            },
            m
        );
    }

    #[test]
    fn optional_parts_off() {
        let s = puzzle();
        let cfg = GenConfig {
            metrics: MetricsConfig {
                random_rollouts: 0,
                ..MetricsConfig::default()
            },
            ..GenConfig::default()
        };
        let m = compute_metrics::<ChaCha20Rng>(&s, &evaluate(&s, &cfg).unwrap(), &cfg);
        assert_eq!((m.random_stuck_rate, m.random_capped_rate), (0.0, 0.0));
        assert_eq!((m.dead_end_ratio_d1, m.dead_end_ratio_d2), (None, None));
    }

    #[test]
    fn rollout_outcomes() {
        // No legal move at all: every rollout is stuck.
        let p = Params {
            n_colors: 2,
            capacity: 2,
            n_empty: 0,
        };
        let s = State::from_tubes(p, &[&[0, 1][..], &[1, 0]]).unwrap();
        let mc = MetricsConfig::default();
        assert_eq!(
            rollouts::<ChaCha20Rng>(&s, 1, 0, &mc),
            (mc.random_rollouts, 0)
        );
        // Cap of 1 move on a puzzle that needs 3: every rollout is capped.
        let p = Params {
            n_colors: 2,
            capacity: 2,
            n_empty: 1,
        };
        let s = State::from_tubes(p, &[&[0, 1][..], &[1, 0], &[]]).unwrap();
        let mc = MetricsConfig {
            rollout_cap_factor: 0,
            ..mc
        };
        assert_eq!(
            rollouts::<ChaCha20Rng>(&s, 3, 0, &mc),
            (0, mc.random_rollouts)
        );
    }

    #[test]
    fn dead_end_ratio_of_a_trap() {
        // Every state one pour away from this near-solved puzzle is still solvable.
        let p = Params {
            n_colors: 3,
            capacity: 3,
            n_empty: 1,
        };
        let s = State::from_tubes(p, &[&[0, 0, 0][..], &[1, 1, 2], &[2, 2], &[1]]).unwrap();
        let (d1, _) = dead_end_ratios(&s, &SolverLimits::default());
        let d1 = d1.unwrap();
        assert!(d1.abs() < f32::EPSILON, "d1 = {d1}");
    }
}
