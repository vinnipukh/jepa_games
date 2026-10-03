//! Rust errors mapped to Python exceptions.

use pyo3::exceptions::{PyOSError, PyRuntimeError, PyValueError};
use pyo3::{PyErr, create_exception};
use water_sort_core::{GenError, Move, MoveError, PuzzleCodeError, StarError, StateError};

create_exception!(
    jepa_water_sort,
    StarBelowOptimalError,
    PyValueError,
    "The player claims fewer moves than the optimum, which means the solver is wrong."
);
create_exception!(
    jepa_water_sort,
    GenerationError,
    PyRuntimeError,
    "No acceptable puzzle within `max_attempts`."
);

pub fn illegal_move(m: Move, e: MoveError) -> PyErr {
    PyValueError::new_err(format!("illegal move {m}: {e}"))
}

pub fn state(e: StateError) -> PyErr {
    PyValueError::new_err(format!("invalid state: {e}"))
}

pub fn puzzle_code(e: PuzzleCodeError) -> PyErr {
    PyValueError::new_err(format!("invalid puzzle code: {e}"))
}

pub fn stars(e: StarError) -> PyErr {
    StarBelowOptimalError::new_err(e.to_string())
}

pub fn generation(e: GenError) -> PyErr {
    match e {
        GenError::TooManyAttempts { .. } => GenerationError::new_err(e.to_string()),
        GenError::Entropy(_) => PyOSError::new_err(e.to_string()),
        GenError::InvalidParams(_) | GenError::UnsupportedLayout { .. } => {
            PyValueError::new_err(e.to_string())
        }
    }
}

pub fn value(msg: impl Into<String>) -> PyErr {
    PyValueError::new_err(msg.into())
}
