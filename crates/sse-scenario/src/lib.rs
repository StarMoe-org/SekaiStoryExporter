//! # sse-scenario -- scenario parsing
//!
//! ## Responsibilities
//! - `ScenarioData` -> IR
//! - Version adaptation of enums and fields. Values come from
//!   `docs/reverse/versions/<region>-<ver>/constants.yaml` via `cargo xtask codegen`;
//!   they are never hand-written into Rust (see `docs/reverse/workflow.md`)
//! - Corpus-wide scenario statistics (coverage report)
//!
//! ## Not responsible for
//! - Timeline computation or branch flattening
//!
//! ## Allowed dependencies
//! `sse-core`, `sse-ir`, `sse-assets`.
