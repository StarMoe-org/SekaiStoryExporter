//! # sse-export -- readback, encoding and muxing
//!
//! ## Responsibilities
//! - Asynchronous readback pipeline (triple-buffered staging). This is performance-critical
//!   on Windows/PCIe and nearly free on Apple Silicon's unified memory
//! - Output forms (decision Q15, all behind flags): mp4, lossless frame sequence,
//!   BLAKE3 frame hashes, layered output, draw-call dumps (narrowly triggered only)
//! - Offline audio mixing: timeline -> ffmpeg `filter_complex` -> mux with the video stream
//!
//! ## Verification carrier
//! Per-frame BLAKE3 hashes. Comparison never happens at the mp4 level -- see
//! `docs/spec/tolerance.md`.
