//! # sse-assets -- asset sources and caching
//!
//! ## Responsibilities
//! - Read-only loading of SekaiStoryRipper output: `ripper-episode` indexes and
//!   `sse-motion` clips via `ripper-format` (decisions Q25 / Q26)
//! - Reject unknown `format` / `version` values instead of guessing
//! - **Explicit failure with a list of missing assets**, pointing at `ripper rip <selector>`.
//!   Silent degradation is forbidden
//! - Recording the asset version from `ripper.lock.json`
//!
//! ## Not responsible for
//! - Downloading or decrypting anything (decision Q25: sse has no CDN or key handling)
//! - Resolving motion/facial bundles; the episode index is trusted (decision Q29)
//! - Interpreting asset contents (each consuming crate does that)
//!
//! ## Allowed dependencies
//! `sse-core` and the external `ripper-format` (serde types only).
