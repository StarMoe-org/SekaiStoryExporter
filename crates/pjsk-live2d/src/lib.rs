//! # pjsk-live2d -- Cubism Core FFI and Framework reimplementation
//!
//! ## Responsibilities
//! - FFI bindings to Cubism Core (decision Q1). **The only crate allowed to use `unsafe`**
//! - Rust reimplementation of the Framework layer: motion, physics, expression, pose, eye blink
//! - `trait MotionEvaluator` -- Native semantics is the first implementation; see `docs/risks.md` R1
//! - model3 assembly: build a usable model from the scattered moc3 / physics3 / motion3 / textures
//!
//! ## Not responsible for
//! - Drawing. This crate only produces vertices, indices and state; `pjsk-render` draws them
//!
//! ## Allowed dependencies
//! `pjsk-core`, `pjsk-assets`.
