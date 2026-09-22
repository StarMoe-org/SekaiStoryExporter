//! # pjsk-cli -- command-line entry point
//!
//! ## Responsibilities
//! - Subcommand orchestration: fetch / stats / compile / bake / render / export / diff
//! - The shell around `OutputConfig` (decision Q15: output form is a struct; the CLI is
//!   only a thin layer over it)
//! - The preview player (roadmap P7) also mounts here and consumes the same parameter
//!   table as the exporter (decision Q4)
