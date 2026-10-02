//! The `Generator` trait end to end, with a minimal Fisher-Yates generator.

use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::SeedableRng;
use water_sort_core::{
    GenConfig, GenError, GeneratedPuzzle, Generator, Observer, Params, State, attempt_loop,
    canonical_hash, fisher_yates, puzzle_code, replay,
};

struct Toy;

impl Generator for Toy {
    const ID: &'static str = "toy";
    const VERSION: u32 = 1;

    fn variant(&self) -> String {
        "fisher_yates".into()
    }

    fn fresh_seed(&self, now_nanos: u64) -> Result<u64, GenError> {
        Ok(water_sort_core::time_seed(now_nanos, 0))
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
        let (state, attempts, accepted) = attempt_loop(cfg, observer, || {
            let mut units = State::sorted_units(*params);
            fisher_yates(&mut rng, &mut units);
            State::from_fill(*params, &units).expect("valid fill")
        })?;
        Ok(self.assemble::<ChaCha20Rng>(state, seed, attempts, accepted, cfg))
    }
}

#[test]
fn generated_puzzles_are_consistent_and_reproducible() {
    let params = Params::with_colors(4);
    let cfg = GenConfig {
        min_opt: 5,
        ..GenConfig::default()
    };
    for seed in 0..20 {
        let g = Toy.generate(&params, seed, &cfg).unwrap();
        assert_eq!(g, Toy.generate(&params, seed, &cfg).unwrap());
        let (traced, counts) = Toy.generate_traced(&params, seed, &cfg).unwrap();
        assert_eq!(traced, g);
        assert_eq!(counts.total() + 1, g.attempts);
        assert_eq!((g.generator_id, g.generator_version), ("toy", 1));
        assert_eq!(g.generator_variant, "fisher_yates");
        assert_eq!(g.seed, seed);
        assert!(g.opt_moves >= 5 && g.attempts >= 1);
        assert_eq!(g.metrics.opt_moves, g.opt_moves);
        assert_eq!(g.solution.len(), g.opt_moves as usize);
        assert!(replay(&g.state, &g.solution).unwrap().is_solved());
        assert_eq!(g.canonical_hash, canonical_hash(&g.state));
        assert_eq!(puzzle_code::decode(&g.puzzle_code), Ok(g.state));
        assert!(
            serde_json::to_string(&g)
                .unwrap()
                .contains("\"generator_id\":\"toy\"")
        );
    }
}

#[test]
fn errors() {
    let bad = Params {
        n_colors: 0,
        capacity: 4,
        n_empty: 2,
    };
    assert!(matches!(
        Toy.generate(&bad, 0, &GenConfig::default()),
        Err(GenError::InvalidParams(_))
    ));
    let impossible = GenConfig {
        min_opt: 1000,
        max_attempts: 3,
        ..GenConfig::default()
    };
    assert_eq!(
        Toy.generate(&Params::with_colors(3), 0, &impossible),
        Err(GenError::TooManyAttempts { attempts: 3 })
    );
    assert_ne!(Toy.fresh_seed(1).unwrap(), Toy.fresh_seed(2).unwrap());
}
