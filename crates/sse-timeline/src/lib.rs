//! # sse-timeline -- timeline compiler
//!
//! Compiles the relative semantics of the IR into an absolute timeline.
//! **Sole owner of pacing correctness.**
//!
//! ## Responsibilities
//! - Snippet stream -> absolute frame ranges: `Delay` semantics, `Now` concurrency,
//!   `WaitUntilFinished` blocking
//! - Duration sources: actual voice length, effect `Duration`, motion length,
//!   and the heuristic used when a line has no voice
//! - Branch flattening (default: first option; `--branch` / `--all-branches`)
//! - Manual override files (e.g. adding a pause to one line)
//!
//! ## Not responsible for
//! - Any rendering or state evaluation
//!
//! ## Allowed dependencies
//! `sse-core`, `sse-ir`, `sse-assets`.
