//! `generate`: picks the generator (uniform or Turan with a strategy) and runs it.

use std::time::{SystemTime, UNIX_EPOCH};

use pyo3::prelude::*;
use turan_water_sort::{Turan, TuranStrategy};
use uniform_water_sort::Uniform;
use water_sort_core::{GenConfig, GenError, GeneratedPuzzle, Generator, Layout, Params};

use crate::errors;
use crate::types::{ParamsArg, PyGenConfig, PyPuzzle, config_or_default, parse_layout};

/// A configured generator.
#[derive(Clone, Copy)]
pub enum AnyGenerator {
    Uniform(Uniform),
    Turan(Turan),
}

impl AnyGenerator {
    /// `generator` is `"uniform"` or `"turan"`; `strategy` (Turan only) is a strategy name with
    /// optional arguments, see [`parse_strategy`].
    pub fn new(generator: &str, layout: &str, strategy: Option<&str>) -> PyResult<Self> {
        let layout = parse_layout(layout)?;
        match generator {
            "uniform" => {
                if strategy.is_some() {
                    return Err(errors::value(
                        "strategy only applies to the turan generator",
                    ));
                }
                Ok(Self::Uniform(Uniform::new(layout)))
            }
            "turan" => {
                let strategy = strategy.map_or_else(
                    || Ok(TuranStrategy::default()),
                    |s| parse_strategy(s, layout),
                )?;
                Ok(Self::Turan(Turan::new(strategy, layout)))
            }
            other => Err(errors::value(format!(
                "unknown generator {other:?} (expected \"uniform\" or \"turan\")"
            ))),
        }
    }

    pub const fn layout(&self) -> Layout {
        match self {
            Self::Uniform(g) => g.layout,
            Self::Turan(g) => g.layout,
        }
    }

    pub fn variant(&self) -> String {
        match self {
            Self::Uniform(g) => g.variant(),
            Self::Turan(g) => g.variant(),
        }
    }

    pub fn fresh_seed(&self) -> Result<u64, GenError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX));
        match self {
            Self::Uniform(g) => g.fresh_seed(now),
            Self::Turan(g) => g.fresh_seed(now),
        }
    }

    pub fn generate(
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

    pub fn puzzle(&self, params: &Params, seed: u64, cfg: &GenConfig) -> PyResult<PyPuzzle> {
        self.generate(params, seed, cfg)
            .map(|p| PyPuzzle::from_generated(p, self.layout()))
            .map_err(errors::generation)
    }
}

/// Parses a Turan strategy spec ([`turan_water_sort::parse_strategy`]).
pub fn parse_strategy(spec: &str, layout: Layout) -> PyResult<TuranStrategy> {
    turan_water_sort::parse_strategy(spec, layout).map_err(|e| errors::value(e.0))
}

/// Generates one puzzle. With `seed=None` the generator picks a fresh seed (OS entropy for
/// uniform, the time seed for Turan, D1); the seed is recorded in the result either way.
/// Releases the GIL.
#[pyfunction]
#[pyo3(signature = (generator, params, seed = None, config = None, strategy = None, layout = "standard"))]
pub fn generate(
    py: Python<'_>,
    generator: &str,
    params: ParamsArg,
    seed: Option<u64>,
    config: Option<PyGenConfig>,
    strategy: Option<&str>,
    layout: &str,
) -> PyResult<PyPuzzle> {
    let g = AnyGenerator::new(generator, layout, strategy)?;
    let params = params.get()?;
    let cfg = config_or_default(config);
    let seed = match seed {
        Some(s) => s,
        None => g.fresh_seed().map_err(errors::generation)?,
    };
    py.detach(|| g.puzzle(&params, seed, &cfg))
}

/// The `generator_variant` string a configured generator records.
#[pyfunction]
#[pyo3(signature = (generator, strategy = None, layout = "standard"))]
pub fn variant(generator: &str, strategy: Option<&str>, layout: &str) -> PyResult<String> {
    Ok(AnyGenerator::new(generator, layout, strategy)?.variant())
}

/// A fresh seed from the generator's own seed source.
#[pyfunction]
#[pyo3(signature = (generator, strategy = None, layout = "standard"))]
pub fn fresh_seed(generator: &str, strategy: Option<&str>, layout: &str) -> PyResult<u64> {
    AnyGenerator::new(generator, layout, strategy)?
        .fresh_seed()
        .map_err(errors::generation)
}
