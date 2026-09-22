//! # pjsk-fidelity -- fidelity comparison
//!
//! The executor of `docs/spec/tolerance.md`.
//!
//! ## Responsibilities
//! - T1 (cross-platform) and T2 (against the game) comparisons
//! - **Difference-shape classifier**: edge band / block / isolated point / global offset.
//!   This is the core of the crate and matters more than the thresholds
//! - Difference heat maps, per-layer attribution, report generation
//!
//! ## Design note
//! The output is not a single number but a shape classification plus a location.
//! A pure magnitude threshold will swallow real bugs -- for example a `max diff = 1`
//! spread evenly over the dialogue box is almost certainly a mis-transcribed colour.
