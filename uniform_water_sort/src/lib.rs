//! Uniform Water Sort generator: Fisher-Yates shuffle on a seeded `ChaCha20Rng`, validated by
//! rejection sampling against the `water_sort_core` solver.
//!
//! Every attempt is uniform over the fills of the chosen [`Layout`], and a rejection continues
//! the same RNG stream, so accepted puzzles are uniform over the labeled configurations that pass
//! [`water_sort_core::evaluate`] (D2), and `(params, seed, GenConfig, layout, VERSION)` fully
//! determines the result.
//!
//! - [`Layout::Standard`]: Fisher-Yates over the units, then the first `n_colors` tubes are
//!   filled. Bit-identical to Phase 2.
//! - [`Layout::Distributed`] (D14): an exactly uniform height vector, then Fisher-Yates over the
//!   units, packed from the bottom. Every height vector admits the same number of unit
//!   arrangements, so this is uniform over all labeled distributed states.

use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use water_sort_core::{
    GenConfig, GenError, GeneratedPuzzle, Generator, HeightCounts, Layout, Observer, Params, State,
    attempt_loop, fisher_yates,
};

/// The uniform generator. Everything else comes from the seed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Uniform {
    pub layout: Layout,
}

impl Uniform {
    pub const fn new(layout: Layout) -> Self {
        Self { layout }
    }
}

impl Generator for Uniform {
    const ID: &'static str = "uniform";
    const VERSION: u32 = 1;

    /// `"fisher_yates"` for the standard layout (unchanged from Phase 2), otherwise
    /// `"fisher_yates(layout=distributed)"`.
    fn variant(&self) -> String {
        match self.layout {
            Layout::Standard => "fisher_yates".into(),
            Layout::Distributed => "fisher_yates(layout=distributed)".into(),
        }
    }

    /// 64 bits of OS entropy; `now_nanos` is ignored.
    fn fresh_seed(&self, _now_nanos: u64) -> Result<u64, GenError> {
        getrandom::u64().map_err(|e| GenError::Entropy(e.to_string()))
    }

    fn generate_observed<O: Observer>(
        &self,
        params: &Params,
        seed: u64,
        cfg: &GenConfig,
        observer: &mut O,
    ) -> Result<GeneratedPuzzle, GenError> {
        params.validate()?;
        let mut rng = ChaCha20Rng::seed_from_u64(seed);
        let sorted = State::sorted_units(*params);
        let (state, attempts, accepted) = match self.layout {
            Layout::Standard => attempt_loop(cfg, observer, || {
                let mut units = sorted.clone();
                fisher_yates(&mut rng, &mut units);
                State::from_fill(*params, &units).expect("a shuffled fill is a valid state")
            })?,
            Layout::Distributed => {
                let heights = HeightCounts::new(*params);
                attempt_loop(cfg, observer, || {
                    let h = heights.sample(&mut rng);
                    let mut units = sorted.clone();
                    fisher_yates(&mut rng, &mut units);
                    State::from_heights(*params, &h, &units)
                        .expect("a valid height vector with shuffled units is a valid state")
                })?
            }
        };
        Ok(self.assemble::<ChaCha20Rng>(state, seed, attempts, accepted, cfg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use water_sort_core::{Evaluation, RejectionCounts, replay};

    const STANDARD: Uniform = Uniform::new(Layout::Standard);

    const P: Params = Params {
        n_colors: 4,
        capacity: 4,
        n_empty: 2,
    };

    #[test]
    fn identity() {
        assert_eq!((Uniform::ID, Uniform::VERSION), ("uniform", 1));
        assert_eq!(STANDARD.variant(), "fisher_yates");
        assert_eq!(
            Uniform::new(Layout::Distributed).variant(),
            "fisher_yates(layout=distributed)"
        );
        let seeds: Vec<u64> = (0..4).map(|_| STANDARD.fresh_seed(0).unwrap()).collect();
        assert!(seeds.windows(2).any(|w| w[0] != w[1]));
    }

    #[test]
    fn generates_valid_reproducible_puzzles() {
        let cfg = GenConfig {
            min_opt: 6,
            ..GenConfig::default()
        };
        for seed in 0..10 {
            let g = STANDARD.generate(&P, seed, &cfg).unwrap();
            assert_eq!(g, STANDARD.generate(&P, seed, &cfg).unwrap());
            assert_eq!((g.generator_id, g.generator_version), ("uniform", 1));
            assert_eq!(g.seed, seed);
            assert!(g.opt_moves >= 6);
            assert_eq!(g.solution.len(), g.opt_moves as usize);
            assert!(replay(&g.state, &g.solution).unwrap().is_solved());
            for i in 0..usize::from(P.n_colors) {
                assert!(g.state.is_tube_full(i));
            }
            for i in usize::from(P.n_colors)..P.n_tubes() {
                assert!(g.state.is_tube_empty(i));
            }
        }
    }

    #[test]
    fn distributed_puzzles() {
        let gen_d = Uniform::new(Layout::Distributed);
        let cfg = GenConfig::default();
        let mut non_standard = 0;
        for seed in 0..20 {
            let g = gen_d.generate(&P, seed, &cfg).unwrap();
            assert_eq!(g, gen_d.generate(&P, seed, &cfg).unwrap());
            assert_eq!(g.generator_variant, "fisher_yates(layout=distributed)");
            assert!(g.state.layout_matches(Layout::Distributed));
            assert!(replay(&g.state, &g.solution).unwrap().is_solved());
            non_standard += usize::from(!g.state.layout_matches(Layout::Standard));
        }
        assert!(non_standard >= 18, "{non_standard}");
    }

    #[test]
    fn traced_counts_every_rejection() {
        // n_empty = 1 rejects often (unsolvable fills).
        let p = Params {
            n_colors: 5,
            capacity: 3,
            n_empty: 1,
        };
        let cfg = GenConfig::default();
        let mut seen_rejections = false;
        for seed in 0..20 {
            let (g, counts) = STANDARD.generate_traced(&p, seed, &cfg).unwrap();
            assert_eq!(g, STANDARD.generate(&p, seed, &cfg).unwrap());
            assert_eq!(counts.total() + 1, g.attempts);
            seen_rejections |= counts.unsolvable > 0;
        }
        assert!(seen_rejections);
    }

    /// Counts calls, to check the observer sees every attempt exactly once.
    #[derive(Default)]
    struct Calls {
        before: u32,
        after: u32,
        counts: RejectionCounts,
    }

    impl Observer for Calls {
        fn before_attempt(&mut self) {
            assert_eq!(self.before, self.after);
            self.before += 1;
        }
        fn after_attempt(&mut self, e: &Evaluation) {
            self.after += 1;
            self.counts.after_attempt(e);
        }
    }

    #[test]
    fn errors() {
        let bad = Params { n_colors: 0, ..P };
        assert!(matches!(
            STANDARD.generate(&bad, 0, &GenConfig::default()),
            Err(GenError::InvalidParams(_))
        ));
        let impossible = GenConfig {
            min_opt: 1000,
            max_attempts: 5,
            ..GenConfig::default()
        };
        let mut calls = Calls::default();
        assert_eq!(
            STANDARD.generate_observed(&P, 0, &impossible, &mut calls),
            Err(GenError::TooManyAttempts { attempts: 5 })
        );
        assert_eq!((calls.before, calls.after), (5, 5));
        assert_eq!(calls.counts.total(), 5);
    }
}
