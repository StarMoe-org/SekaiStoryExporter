//! # sse-core -- foundation types and determinism utilities
//!
//! Lowest-level facilities shared across the workspace. **Contains no business logic.**
//!
//! ## Responsibilities
//! - Coordinate-space and unit newtypes (see `docs/spec/coordinate-systems.md`)
//! - `det_math`: transcendental functions that are bit-identical across platforms
//!   (backed by the `libm` crate, never `std` -- see `docs/conventions/determinism.md` D-4)
//! - Deterministic helpers such as `fs::read_dir_sorted`
//! - Time-base types: `SimFrame`, `FrameNo`, `Seconds`, `TimeBase` (see decisions Q33 / Q34)
//!
//! ## Not responsible for
//! - Anything specific to PJSK
//!
//! ## Allowed dependencies
//! None on `sse-*`. This is the leaf of the dependency graph.
