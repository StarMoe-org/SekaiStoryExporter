//! # pjsk-text — SDF 文本
//!
//! ## 职责
//! - TMP FontAsset 解析：SDF atlas、glyph 表、kerning 表
//! - TMP 排版语义：行距、字距、CJK 换行与禁则、富文本标签
//! - 打字机效果（按 `maxVisibleCharacters` 逐字，非逐像素揭示）
//! - TMP_SDF shader 参数的解析与传递（shader 本体在 `pjsk-render`）
//! - fallback：用游戏自带 TTF 生成 SDF（Q11；力争不触发）
//!
//! ## 允许依赖
//! `pjsk-core`、`pjsk-assets`、`pjsk-ugui`。
