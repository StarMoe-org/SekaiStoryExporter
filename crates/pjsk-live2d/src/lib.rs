//! # pjsk-live2d — Cubism Core FFI + Framework 重写
//!
//! ## 职责
//! - Cubism Core 的 FFI 绑定（Q1：可 FFI，唯一允许 `unsafe` 的地方）
//! - Framework 层的 Rust 重写：motion / physics / expression / pose / eye-blink
//! - `trait MotionEvaluator`：Native 语义是第一个实现（见 `docs/risks.md` R1）
//! - model3 组装：从散落的 moc3 / physics3 / motion3 / 贴图拼出可用模型
//!
//! ## 不负责
//! - 绘制（只输出顶点/索引/状态，由 `pjsk-render` 绘制）
//!
//! ## 允许依赖
//! `pjsk-core`、`pjsk-assets`。
