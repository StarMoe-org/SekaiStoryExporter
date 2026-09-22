//! # pjsk-ir -- intermediate representation
//!
//! The first seam of the project: the contract between the parsing layer and the
//! compilation layer. Format spec: `docs/spec/ir.md`.
//!
//! ## Responsibilities
//! - IR data structures, (de)serialisation, and version field
//! - Backwards-compatibility handling across IR versions
//! - Representation of `unsupported` events -- decision Q10 covers 2D dialogue only,
//!   so unsupported content must retain enough information to be filled in later
//!
//! ## Not responsible for
//! - Parsing `ScenarioData` (that is `pjsk-scenario`)
//! - Any time computation (that is `pjsk-timeline`)
//!
//! ## Allowed dependencies
//! `pjsk-core` only.
