//! # sse-text -- SDF text
//!
//! ## Responsibilities
//! - TextMeshPro FontAsset parsing: SDF atlas, glyph table, kerning table
//! - TMP layout semantics: line spacing, character spacing, CJK line breaking and
//!   kinsoku rules, rich-text tags
//! - Typewriter reveal (TMP advances `maxVisibleCharacters`; it is not a per-pixel wipe)
//! - Parsing and forwarding TMP_SDF material parameters (the shader itself lives in `sse-render`)
//! - Fallback: generate SDF from the game's own TTF (decision Q11; we aim never to hit this)
//!
//! ## Allowed dependencies
//! `sse-core`, `sse-assets`, `sse-ugui`.
