//! # sse-assets -- asset sources and caching
//!
//! ## Responsibilities
//! - Asset source abstraction: Web Assets Source / local directory (decision Q17)
//! - Manifest and content-addressed cache
//! - **Explicit failure with a list of missing assets.** Silent degradation is forbidden
//! - Asset version pinning (`assets.lock`)
//!
//! ## Not responsible for
//! - Interpreting asset contents (each consuming crate does that)
//!
//! ## Allowed dependencies
//! `sse-core` only.
