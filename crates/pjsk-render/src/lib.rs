//! # pjsk-render -- Pass 2: stateless rendering
//!
//! ## Responsibilities
//! - `render(frame_n)`: takes one frame of the parameter table, produces pixels. **Must be stateless**
//! - wgpu pipelines, SSAA, layered output (decision Q15, off by default)
//! - Shaders: TMP_SDF, Cubism, and the special effects
//!
//! ## Hard rule
//! > **This crate must not depend on `pjsk-timeline` or `pjsk-scenario`.**
//! > Rendering only ever sees the parameter table. That dependency constraint is the
//! > structural guarantee of Pass 2's statelessness; breaking it breaks the Q14 architecture.
//!
//! ## Determinism
//! This crate is the main battleground for `docs/conventions/determinism.md`.
//! Every R-level rule applies here.
