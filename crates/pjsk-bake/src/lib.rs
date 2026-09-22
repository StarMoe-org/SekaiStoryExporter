//! # pjsk-bake -- Pass 1: state baking
//!
//! Walks the timeline sequentially, updating state only -- no rendering -- and emits a
//! per-frame parameter table. Format spec: `docs/spec/param-table.md`.
//!
//! ## Responsibilities
//! - Drive the Live2D state machine, UI state, background and effect state
//! - Emit a complete state snapshot per frame (the parameter table)
//! - delta + zstd encoding
//!
//! ## Why this crate exists
//! The parameter table is one intermediate layer that buys four things at once:
//! observability (compare numerically against parameters dumped from the game),
//! statelessness for Pass 2, parallel rendering, and timeline seeking.
//! See `docs/architecture.md`.
//!
//! ## Not responsible for
//! - Any GPU work
