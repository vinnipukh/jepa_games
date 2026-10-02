//! Water Sort game core: state, move rules, solver, star metric, canonical hash and the
//! `Generator` trait. Every other crate takes its game logic from here.

#![forbid(unsafe_code)]

pub mod params;
pub mod state;

pub use params::{MAX_CAP, MAX_TUBES, Params, ParamsError};
pub use state::{EMPTY, State, StateError};
