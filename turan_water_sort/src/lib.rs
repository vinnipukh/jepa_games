//! Turan Water Sort generator (D1): strategy-based construction on a time-seeded
//! `ChaCha20Rng`, validated by the `water_sort_core` solver.
//!
//! - [`TuranStrategy::Scramble`] (default): a random walk of reverse pours from the solved state.
//!   Solvable by construction, with longer same-color runs than a uniform fill.
//! - [`TuranStrategy::Constrained`]: the uniform construction of the layout, rejecting any fill
//!   with two vertically adjacent units of the same color.
//!
//! Both work in either [`Layout`] (D14). All game rules come from core: reverse moves
//! ([`reverse_moves`], [`unapply`]), state construction, and validation through
//! [`try_attempt_loop`]. `(params, seed, GenConfig, strategy, layout, VERSION)` fully determines
//! the result; the strategy and layout are recorded in [`Generator::variant`].

use std::sync::atomic::{AtomicU64, Ordering};

use rand_chacha::ChaCha20Rng;
use rand_core::{Rng, SeedableRng};
use water_sort_core::{
    Evaluation, GenConfig, GenError, GeneratedPuzzle, Generator, HeightCounts, Layout, Observer,
    Params, Rejection, ReverseMove, State, bounded_u32, fisher_yates, reverse_moves, time_seed,
    try_attempt_loop, unapply,
};

/// How a Turan candidate is built.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TuranStrategy {
    /// Random walk of `steps` reverse pours from the solved state. For [`Layout::Standard`] the
    /// walk continues for at most `max_extra_steps` more pours until it reaches a state with
    /// `n_colors` full and `n_empty` empty tubes; otherwise the attempt is rejected
    /// ([`Rejection::Construction`]).
    Scramble { steps: u32, max_extra_steps: u32 },
    /// The uniform construction of the layout, rejecting (as [`Rejection::Construction`]) any
    /// fill with two vertically adjacent units of the same color.
    Constrained,
}

impl TuranStrategy {
    pub const DEFAULT_STEPS: u32 = 40;
    pub const DEFAULT_MAX_EXTRA_STEPS: u32 = 100;

    /// `Scramble` with `steps` and the default `max_extra_steps`.
    pub const fn scramble(steps: u32) -> Self {
        Self::Scramble {
            steps,
            max_extra_steps: Self::DEFAULT_MAX_EXTRA_STEPS,
        }
    }
}

impl Default for TuranStrategy {
    fn default() -> Self {
        Self::scramble(Self::DEFAULT_STEPS)
    }
}

/// The Turan generator.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Turan {
    pub strategy: TuranStrategy,
    pub layout: Layout,
}

impl Turan {
    pub const fn new(strategy: TuranStrategy, layout: Layout) -> Self {
        Self { strategy, layout }
    }
}

/// Process-wide counter that keeps time seeds distinct within one clock tick (D1).
static SEED_COUNTER: AtomicU64 = AtomicU64::new(0);

impl Generator for Turan {
    const ID: &'static str = "turan";
    const VERSION: u32 = 1;

    /// E.g. `"scramble(steps=40,max_extra_steps=100,layout=standard)"`,
    /// `"scramble(steps=40,layout=distributed)"`, `"constrained(layout=standard)"`.
    /// `max_extra_steps` only affects the standard layout, so only that variant records it.
    fn variant(&self) -> String {
        let layout = self.layout;
        match (self.strategy, layout) {
            (
                TuranStrategy::Scramble {
                    steps,
                    max_extra_steps,
                },
                Layout::Standard,
            ) => {
                format!("scramble(steps={steps},max_extra_steps={max_extra_steps},layout={layout})")
            }
            (TuranStrategy::Scramble { steps, .. }, Layout::Distributed) => {
                format!("scramble(steps={steps},layout={layout})")
            }
            (TuranStrategy::Constrained, _) => format!("constrained(layout={layout})"),
        }
    }

    /// `time_seed(now_nanos, counter)` with a process-wide counter: two calls never return the
    /// same seed for the same `now_nanos` (D1). Never fails.
    fn fresh_seed(&self, now_nanos: u64) -> Result<u64, GenError> {
        Ok(time_seed(
            now_nanos,
            SEED_COUNTER.fetch_add(1, Ordering::Relaxed),
        ))
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
        let params = *params;
        let mut observer = NeverUnsolvable {
            inner: observer,
            check: matches!(self.strategy, TuranStrategy::Scramble { .. }),
        };
        let (state, attempts, accepted) = match self.strategy {
            TuranStrategy::Scramble {
                steps,
                max_extra_steps,
            } => try_attempt_loop(cfg, &mut observer, || {
                scramble(&mut rng, params, steps, max_extra_steps, self.layout).state
            })?,
            TuranStrategy::Constrained => {
                let heights = HeightCounts::new(params);
                try_attempt_loop(cfg, &mut observer, || {
                    let state = uniform_fill(&mut rng, params, self.layout, &heights);
                    no_adjacent_same_color(&state).then_some(state)
                })?
            }
        };
        Ok(self.assemble::<ChaCha20Rng>(state, seed, attempts, accepted, cfg))
    }
}

/// Forwards to the caller's observer; for scrambles, asserts (debug builds) that no candidate is
/// unsolvable, since every one is reached by reverse pours from a solved state.
struct NeverUnsolvable<'a, O> {
    inner: &'a mut O,
    check: bool,
}

impl<O: Observer> Observer for NeverUnsolvable<'_, O> {
    fn before_attempt(&mut self) {
        self.inner.before_attempt();
    }

    fn after_attempt(&mut self, evaluation: &Evaluation) {
        debug_assert!(
            !(self.check && evaluation.outcome == Err(Rejection::Unsolvable)),
            "a scrambled state is solvable by construction"
        );
        self.inner.after_attempt(evaluation);
    }
}

/// One scramble candidate, with what it cost.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScrambleOutcome {
    /// The candidate, or `None` if the standard layout was not reached in `max_extra_steps`.
    pub state: Option<State>,
    /// Reverse pours applied after the first `steps` (always 0 for [`Layout::Distributed`]).
    pub extra_steps: u32,
    /// Whether the walk stopped early at a state with no reverse move at all.
    pub dead_end: bool,
}

/// Builds one scramble candidate, continuing `rng`'s stream (the construction of
/// [`TuranStrategy::Scramble`], public so that measurements can see `extra_steps`):
///
/// 1. start from [`State::solved`];
/// 2. apply `steps` reverse pours, each drawn with [`bounded_u32`] from [`reverse_moves`] (fixed
///    order), excluding the exact inverse of the previous pour unless it is the only one. A state
///    with no reverse move at all ends the walk early;
/// 3. [`Layout::Standard`]: keep pouring until there are exactly `n_colors` full and `n_empty`
///    empty tubes, at most `max_extra_steps` more (else `state` is `None`), then put the empty
///    tubes last. [`Layout::Distributed`]: nothing;
/// 4. shuffle the color labels, then the tube order (the full tubes only for `Standard`, all
///    tubes for `Distributed`), both with [`fisher_yates`].
///
/// # Panics
///
/// If `params` is invalid.
pub fn scramble<R: Rng + ?Sized>(
    rng: &mut R,
    params: Params,
    steps: u32,
    max_extra_steps: u32,
    layout: Layout,
) -> ScrambleOutcome {
    let mut walk = Walk {
        state: State::solved(params).expect("valid params"),
        previous: None,
        dead_end: false,
    };
    for _ in 0..steps {
        if !walk.step(rng) {
            break;
        }
    }
    let mut extra_steps = 0;
    if layout == Layout::Standard {
        while !has_standard_heights(&walk.state) {
            if extra_steps == max_extra_steps || !walk.step(rng) {
                return ScrambleOutcome {
                    state: None,
                    extra_steps,
                    dead_end: walk.dead_end,
                };
            }
            extra_steps += 1;
        }
    }
    ScrambleOutcome {
        state: Some(shuffle_symmetries(rng, &walk.state, layout)),
        extra_steps,
        dead_end: walk.dead_end,
    }
}

struct Walk {
    state: State,
    previous: Option<ReverseMove>,
    dead_end: bool,
}

impl Walk {
    /// One random reverse pour; `false` (and `dead_end`) if there is none.
    fn step<R: Rng + ?Sized>(&mut self, rng: &mut R) -> bool {
        let moves = reverse_moves(&self.state);
        let inverse = self
            .previous
            .map(|r| ReverseMove::new(r.to, r.from, r.count));
        let mut choices: Vec<ReverseMove> = moves
            .iter()
            .copied()
            .filter(|&r| Some(r) != inverse)
            .collect();
        if choices.is_empty() {
            choices = moves;
        }
        if choices.is_empty() {
            self.dead_end = true;
            return false;
        }
        let n = u32::try_from(choices.len()).expect("few reverse moves");
        let r = choices[bounded_u32(rng, n) as usize];
        self.state = unapply(&self.state, r).expect("listed reverse moves are valid");
        self.previous = Some(r);
        true
    }
}

/// Exactly `n_colors` full tubes and `n_empty` empty ones, in any positions.
fn has_standard_heights(s: &State) -> bool {
    let p = s.params();
    let full = (0..s.n_tubes()).filter(|&i| s.is_tube_full(i)).count();
    let empty = (0..s.n_tubes()).filter(|&i| s.is_tube_empty(i)).count();
    full == usize::from(p.n_colors) && empty == usize::from(p.n_empty)
}

/// Relabels colors and permutes tubes at random: symmetries of the game, not moves. For
/// `Standard`, the empty tubes go last first and only the full tubes are permuted.
fn shuffle_symmetries<R: Rng + ?Sized>(rng: &mut R, s: &State, layout: Layout) -> State {
    let params = s.params();
    let mut labels: Vec<u8> = (0..params.n_colors).collect();
    fisher_yates(rng, &mut labels);
    let mut tubes: Vec<Vec<u8>> = s
        .tubes()
        .into_iter()
        .map(|t| t.into_iter().map(|c| labels[usize::from(c)]).collect())
        .collect();
    match layout {
        Layout::Standard => {
            // Stable: full tubes in walk order, then the empty ones.
            tubes.sort_by_key(Vec::is_empty);
            fisher_yates(rng, &mut tubes[..usize::from(params.n_colors)]);
        }
        Layout::Distributed => fisher_yates(rng, &mut tubes),
    }
    State::from_tubes(params, &tubes).expect("a relabeled permutation of a state is valid")
}

/// The uniform generator's construction for `layout` (heights first for `Distributed`).
fn uniform_fill<R: Rng + ?Sized>(
    rng: &mut R,
    params: Params,
    layout: Layout,
    heights: &HeightCounts,
) -> State {
    let mut units = State::sorted_units(params);
    match layout {
        Layout::Standard => {
            fisher_yates(rng, &mut units);
            State::from_fill(params, &units).expect("a shuffled fill is a valid state")
        }
        Layout::Distributed => {
            let h = heights.sample(rng);
            fisher_yates(rng, &mut units);
            State::from_heights(params, &h, &units).expect("a valid height vector is valid")
        }
    }
}

/// No tube holds two vertically adjacent units of one color: every unit is its own segment.
pub fn no_adjacent_same_color(s: &State) -> bool {
    s.segments() as usize == s.params().n_units()
}

#[cfg(test)]
mod tests {
    use super::*;
    use water_sort_core::{RejectionCounts, replay};

    const P: Params = Params {
        n_colors: 4,
        capacity: 4,
        n_empty: 2,
    };

    fn all() -> Vec<Turan> {
        let mut out = Vec::new();
        for layout in Layout::ALL {
            out.push(Turan::new(TuranStrategy::default(), layout));
            out.push(Turan::new(TuranStrategy::Constrained, layout));
        }
        out
    }

    #[test]
    fn identity() {
        assert_eq!((Turan::ID, Turan::VERSION), ("turan", 1));
        let variants: Vec<String> = all().iter().map(Generator::variant).collect();
        assert_eq!(
            variants,
            [
                "scramble(steps=40,max_extra_steps=100,layout=standard)",
                "constrained(layout=standard)",
                "scramble(steps=40,layout=distributed)",
                "constrained(layout=distributed)",
            ]
        );
    }

    #[test]
    fn fresh_seeds_differ_at_equal_time() {
        let t = Turan::default();
        let mut seeds: Vec<u64> = (0..1000).map(|_| t.fresh_seed(123).unwrap()).collect();
        seeds.sort_unstable();
        seeds.dedup();
        assert_eq!(seeds.len(), 1000);
    }

    #[test]
    fn generates_valid_reproducible_puzzles() {
        let cfg = GenConfig::default();
        for t in all() {
            for seed in 0..10 {
                let (g, counts) = t.generate_traced(&P, seed, &cfg).unwrap();
                assert_eq!(g, t.generate(&P, seed, &cfg).unwrap());
                assert_eq!(g.generator_variant, t.variant());
                assert_eq!(counts.unsolvable, 0, "{t:?} seed {seed}");
                assert_eq!(counts.total() + 1, g.attempts);
                assert!(g.state.layout_matches(t.layout), "{t:?} seed {seed}");
                assert!(replay(&g.state, &g.solution).unwrap().is_solved());
                if t.strategy == TuranStrategy::Constrained {
                    assert!(no_adjacent_same_color(&g.state));
                }
            }
        }
    }

    #[test]
    fn scramble_walks_and_returns_to_standard() {
        let mut rng = ChaCha20Rng::seed_from_u64(5);
        let mut returned = 0;
        for _ in 0..200 {
            // Walks absorbed in a state with no reverse move (or ping-ponging one unit between
            // two tubes) never return; the others must match the layout.
            let out = scramble(&mut rng, P, 20, 100, Layout::Standard);
            if let Some(s) = out.state {
                assert!(s.layout_matches(Layout::Standard));
                returned += 1;
            }
        }
        assert!(returned > 0);
        // Zero steps: the solved state, relabeled.
        let out = scramble(&mut rng, P, 0, 0, Layout::Standard);
        assert_eq!(out.extra_steps, 0);
        assert!(out.state.unwrap().is_solved());
        // Distributed never fails and takes no extra steps.
        for _ in 0..50 {
            let d = scramble(&mut rng, P, 20, 0, Layout::Distributed);
            assert_eq!(d.extra_steps, 0);
            assert!(d.state.is_some());
        }
    }

    #[test]
    fn dead_end_is_a_state_without_reverse_moves() {
        let mut rng = ChaCha20Rng::seed_from_u64(9);
        let out = (0..100)
            .map(|_| scramble(&mut rng, P, 1000, 0, Layout::Distributed))
            .find(|o| o.dead_end)
            .expect("most long walks are absorbed");
        assert_eq!(reverse_moves(&out.state.unwrap()), Vec::new());
    }

    #[test]
    fn construction_failures_count_as_attempts() {
        let t = Turan::new(
            TuranStrategy::Scramble {
                steps: 21,
                max_extra_steps: 0,
            },
            Layout::Standard,
        );
        let cfg = GenConfig::default();
        let mut counts = RejectionCounts::default();
        let mut seen = false;
        for seed in 0..20 {
            let g = t.generate_observed(&P, seed, &cfg, &mut counts).unwrap();
            seen |= g.attempts > 1;
        }
        assert!(seen && counts.construction > 0, "{counts:?}");
        // One color in the standard layout can never avoid adjacent units.
        let one = Params {
            n_colors: 1,
            capacity: 3,
            n_empty: 1,
        };
        let short = GenConfig {
            max_attempts: 5,
            ..cfg
        };
        assert_eq!(
            Turan::new(TuranStrategy::Constrained, Layout::Standard).generate(&one, 0, &short),
            Err(GenError::TooManyAttempts { attempts: 5 })
        );
    }

    #[test]
    fn invalid_params() {
        let bad = Params { n_colors: 0, ..P };
        assert!(matches!(
            Turan::default().generate(&bad, 0, &GenConfig::default()),
            Err(GenError::InvalidParams(_))
        ));
    }
}
