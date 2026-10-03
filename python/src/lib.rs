//! `jepa_water_sort._native`: the Python binding of `water_sort_core` and the two generators.
//!
//! Thin wrappers only. Every rule (moves, solving, stars, hashing, generation) runs in the Rust
//! crates, so Python results are bit-identical to Rust ones.

#![forbid(unsafe_code)]
// PyO3 signatures take arguments by value and return `PyResult` everywhere.
// `#[pymethods]` on frozen classes always take `&self`, even for small `Copy` wrappers.
#![allow(
    clippy::needless_pass_by_value,
    clippy::missing_errors_doc,
    clippy::trivially_copy_pass_by_ref,
    clippy::wrong_self_convention
)]

mod batch;
mod episode;
mod errors;
mod generate;
mod types;

use pyo3::prelude::*;

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("EMPTY", water_sort_core::EMPTY)?;
    m.add(
        "StarBelowOptimalError",
        py.get_type::<errors::StarBelowOptimalError>(),
    )?;
    m.add("GenerationError", py.get_type::<errors::GenerationError>())?;
    m.add_class::<types::PyParams>()?;
    m.add_class::<types::PyGenConfig>()?;
    m.add_class::<types::PyState>()?;
    m.add_class::<types::PyPuzzle>()?;
    m.add_class::<types::PySolveResult>()?;
    m.add_function(wrap_pyfunction!(types::step, m)?)?;
    m.add_function(wrap_pyfunction!(types::legal_moves, m)?)?;
    m.add_function(wrap_pyfunction!(types::action_mask, m)?)?;
    m.add_function(wrap_pyfunction!(types::solve, m)?)?;
    m.add_function(wrap_pyfunction!(types::stars, m)?)?;
    m.add_function(wrap_pyfunction!(types::canonical_hash, m)?)?;
    m.add_function(wrap_pyfunction!(types::canonical, m)?)?;
    m.add_function(wrap_pyfunction!(types::tier, m)?)?;
    m.add_function(wrap_pyfunction!(types::splitmix64, m)?)?;
    m.add_function(wrap_pyfunction!(generate::generate, m)?)?;
    m.add_function(wrap_pyfunction!(generate::variant, m)?)?;
    m.add_function(wrap_pyfunction!(generate::fresh_seed, m)?)?;
    m.add_function(wrap_pyfunction!(batch::batch_step, m)?)?;
    m.add_function(wrap_pyfunction!(batch::batch_generate, m)?)?;
    m.add_function(wrap_pyfunction!(episode::env_step, m)?)?;
    m.add_function(wrap_pyfunction!(episode::move_limit, m)?)?;
    m.add_function(wrap_pyfunction!(episode::is_dead_end, m)?)?;
    m.add_function(wrap_pyfunction!(batch::batch_env_step, m)?)?;
    m.add_function(wrap_pyfunction!(batch::batch_action_mask, m)?)?;
    Ok(())
}
