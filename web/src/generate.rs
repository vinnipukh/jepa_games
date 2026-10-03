//! Generation: picks the generator (uniform, or Turan with a strategy) and runs it.

use turan_water_sort::{Turan, TuranStrategy};
use uniform_water_sort::Uniform;
use wasm_bindgen::prelude::*;
use water_sort_core::{
    GenConfig, GenError, GeneratedPuzzle, Generator, Layout, MetricsConfig, Params, SolverLimits,
    Tier, is_supported_in,
};

use crate::puzzle::Puzzle;
use crate::{JsParams, hex, now_ms, parse_layout, parse_seed};

/// Solver state budget for puzzles opened by code (the generation default, D11).
pub const WEB_MAX_STATES: u32 = 5_000_000;
const _: () = assert!(WEB_MAX_STATES as u64 == SolverLimits::DEFAULT_MAX_STATES);

/// A configured generator.
#[derive(Clone, Copy)]
enum AnyGenerator {
    Uniform(Uniform),
    Turan(Turan),
}

impl AnyGenerator {
    fn new(generator: &str, layout: Layout, strategy: Option<&str>) -> Result<Self, JsError> {
        match generator {
            "uniform" => {
                if strategy.is_some_and(|s| !s.is_empty()) {
                    return Err(JsError::new("strategy only applies to the turan generator"));
                }
                Ok(Self::Uniform(Uniform::new(layout)))
            }
            "turan" => {
                let strategy = match strategy.filter(|s| !s.is_empty()) {
                    None => TuranStrategy::default(),
                    Some(s) => turan_water_sort::parse_strategy(s, layout)
                        .map_err(|e| JsError::new(&e.0))?,
                };
                Ok(Self::Turan(Turan::new(strategy, layout)))
            }
            other => Err(JsError::new(&format!(
                "unknown generator {other:?} (expected \"uniform\" or \"turan\")"
            ))),
        }
    }

    fn variant(&self) -> String {
        match self {
            Self::Uniform(g) => g.variant(),
            Self::Turan(g) => g.variant(),
        }
    }

    fn fresh_seed(&self) -> Result<u64, GenError> {
        // Date.now() has millisecond resolution; Turan's counter separates equal readings (D1).
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let now_nanos = (now_ms() * 1e6) as u64;
        match self {
            Self::Uniform(g) => g.fresh_seed(now_nanos),
            Self::Turan(g) => g.fresh_seed(now_nanos),
        }
    }

    fn generate(
        &self,
        params: &Params,
        seed: u64,
        cfg: &GenConfig,
    ) -> Result<GeneratedPuzzle, GenError> {
        match self {
            Self::Uniform(g) => g.generate(params, seed, cfg),
            Self::Turan(g) => g.generate(params, seed, cfg),
        }
    }
}

fn layout_or_default(layout: Option<String>) -> Result<Layout, JsError> {
    layout
        .filter(|l| !l.is_empty())
        .map_or(Ok(Layout::Standard), |l| parse_layout(&l))
}

/// The web's generation settings: the core default without random rollouts (they only fill the
/// metrics, so the puzzle for a seed is the same, as in Python's `WaterSortEnv`), and with a
/// tier's `opt_moves` band when `tier` is given (D16).
pub fn web_config(
    params: &Params,
    layout: Layout,
    tier: Option<Tier>,
) -> Result<GenConfig, JsError> {
    let mut cfg = GenConfig {
        metrics: MetricsConfig {
            random_rollouts: 0,
            ..MetricsConfig::default()
        },
        ..GenConfig::default()
    };
    if let Some(t) = tier {
        let (min_opt, max_opt) = t.opt_band(params, layout).ok_or_else(|| {
            JsError::new("difficulty tiers are only defined for supported params")
        })?;
        cfg.min_opt = min_opt;
        cfg.max_opt = max_opt;
    }
    Ok(cfg)
}

/// The web's `GenConfig` as JSON (the dataset manifests' `gen_config_json` format), so the same
/// puzzle can be generated in Python with `GenConfig.from_json`.
#[wasm_bindgen]
pub fn web_config_json(
    params: &JsParams,
    layout: Option<String>,
    tier: Option<String>,
) -> Result<String, JsError> {
    let layout = layout_or_default(layout)?;
    let tier = parse_tier(tier)?;
    Ok(serde_json::to_string(&web_config(
        &params.core(),
        layout,
        tier,
    )?)?)
}

fn parse_tier(tier: Option<String>) -> Result<Option<Tier>, JsError> {
    tier.filter(|t| !t.is_empty() && t != "any")
        .map(|t| {
            t.parse()
                .map_err(|e: water_sort_core::UnknownTier| JsError::new(&e.to_string()))
        })
        .transpose()
}

fn run(
    g: AnyGenerator,
    layout: Layout,
    params: Params,
    seed: Option<String>,
    cfg: &GenConfig,
) -> Result<Puzzle, JsError> {
    let seed = match seed.filter(|s| !s.trim().is_empty()) {
        Some(s) => parse_seed(&s)?,
        None => g.fresh_seed()?,
    };
    let generated = g.generate(&params, seed, cfg)?;
    Ok(Puzzle::from_generated(generated, layout, cfg))
}

/// Generates one puzzle with the web's settings ([`web_config`]). `seed` is hex; without one
/// the generator picks a fresh seed (OS entropy for uniform, the time seed for Turan, D1), which
/// is recorded in the puzzle either way. `strategy` (Turan only) is a strategy spec such as
/// `"scramble"` or `"scramble(steps=40)"`; `layout` is `"standard"` (default) or
/// `"distributed"`; `tier` (uniform only) is `"easy"`, `"medium"`, `"hard"` or `"any"`
/// (default).
///
/// # Errors
///
/// Unknown names, bad seeds, params outside the supported range, or no acceptable puzzle.
#[wasm_bindgen]
pub fn generate(
    generator: &str,
    params: &JsParams,
    seed: Option<String>,
    strategy: Option<String>,
    layout: Option<String>,
    tier: Option<String>,
) -> Result<Puzzle, JsError> {
    let layout = layout_or_default(layout)?;
    let g = AnyGenerator::new(generator, layout, strategy.as_deref())?;
    let p = params.core();
    if !is_supported_in(&p, layout) {
        return Err(JsError::new(&format!(
            "{} colors, capacity {}, {} empty is outside the supported range for the {layout} \
             layout",
            p.n_colors, p.capacity, p.n_empty
        )));
    }
    let tier = parse_tier(tier)?;
    if tier.is_some() && matches!(g, AnyGenerator::Turan(_)) {
        // The cut points come from uniform's `opt_moves` distribution (D16); several Turan
        // strategies rarely or never reach some bands (D20).
        return Err(JsError::new(
            "difficulty tiers only apply to the uniform generator",
        ));
    }
    let cfg = web_config(&p, layout, tier)?;
    run(g, layout, p, seed, &cfg)
}

/// [`generate`] with an explicit `GenConfig` (JSON, as in dataset manifests and golden files)
/// and no supported-range check. The golden-vector tests use it.
#[wasm_bindgen]
pub fn generate_with_config(
    generator: &str,
    params: &JsParams,
    seed: Option<String>,
    strategy: Option<String>,
    layout: Option<String>,
    config_json: &str,
) -> Result<Puzzle, JsError> {
    let layout = layout_or_default(layout)?;
    let g = AnyGenerator::new(generator, layout, strategy.as_deref())?;
    let cfg: GenConfig = serde_json::from_str(config_json)?;
    run(g, layout, params.core(), seed, &cfg)
}

/// The `generator_variant` string a configured generator records.
#[wasm_bindgen]
pub fn variant(
    generator: &str,
    strategy: Option<String>,
    layout: Option<String>,
) -> Result<String, JsError> {
    Ok(AnyGenerator::new(generator, layout_or_default(layout)?, strategy.as_deref())?.variant())
}

/// A fresh seed (hex) from the generator's own seed source.
#[wasm_bindgen]
pub fn fresh_seed(
    generator: &str,
    strategy: Option<String>,
    layout: Option<String>,
) -> Result<String, JsError> {
    let g = AnyGenerator::new(generator, layout_or_default(layout)?, strategy.as_deref())?;
    Ok(hex(g.fresh_seed()?))
}

/// The Turan strategy names offered for `layout`; `pour_walk` is distributed only (D16).
/// The first one is the default.
#[wasm_bindgen]
pub fn strategies(layout: &str) -> Result<Vec<String>, JsError> {
    let mut names = vec!["reverse_search", "scramble", "constrained"];
    if parse_layout(layout)? == Layout::Distributed {
        names.push("pour_walk");
    }
    Ok(names.into_iter().map(String::from).collect())
}
