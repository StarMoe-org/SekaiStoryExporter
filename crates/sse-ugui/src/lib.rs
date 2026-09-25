//! # sse-ugui -- Unity UGUI layout evaluator
//!
//! ADR-0010 commits to the full resolution range, and this crate is where that cost lands.
//! **Everything here is deterministic mathematics with no unknowns.**
//!
//! ## Responsibilities
//! - Recursive `RectTransform` evaluation (anchor / pivot / offset / sizeDelta / scale / rotation)
//! - All three `CanvasScaler` modes, including
//!   `scaleFactor = 2^lerp(log2(w/refW), log2(h/refH), match)`
//! - LayoutGroup family (two-pass layout), ContentSizeFitter, LayoutElement, AspectRatioFitter
//! - Safe area and aspect clamping
//!
//! ## Not responsible for
//! - Drawing; text layout (that is `sse-text`)
//!
//! ## Testing
//! Asserted against fixtures exported by `tools/dumper` across multiple resolutions,
//! to floating-point precision. See `docs/testing.md` L1.
