//! # sse-live2d -- Cubism Core FFI and Framework-layer reimplementation
//!
//! ## Responsibilities
//! - FFI bindings to Cubism Core (ADR-0003). **The only crate allowed to use `unsafe`**
//! - The game's runtime semantics in Rust (ADR-0007): Unity `AnimationClip` playback with
//!   `PlayableBlender` crossfades, the game's eye-blink controller, and Cubism physics as run by
//!   the SDK for Unity 4.x
//!
//! ## Not responsible for
//! - Drawing. This crate only produces vertices, indices and state; `sse-render` draws them
//!
//! ## Allowed dependencies
//! `sse-core`, `sse-assets`.

pub mod blink;
pub mod core;
pub mod motion;
pub mod physics;

pub use crate::core::{Moc, Model, core_version};

/// Parameter ids the game looks up by name: old id first, new id as fallback.
pub const PARAM_MOUTH_OPEN_Y: [&str; 2] = ["PARAM_MOUTH_OPEN_Y", "ParamMouthOpenY"];
pub const PARAM_EYE_L_OPEN: [&str; 2] = ["PARAM_EYE_L_OPEN", "ParamEyeLOpen"];
pub const PARAM_EYE_R_OPEN: [&str; 2] = ["PARAM_EYE_R_OPEN", "ParamEyeROpen"];
pub const PARAM_BREATH: [&str; 2] = ["PARAM_BREATH", "ParamBreath"];

pub fn find_param(ids: &[String], names: [&str; 2]) -> Option<usize> {
    names.iter().find_map(|n| ids.iter().position(|id| id == n))
}
