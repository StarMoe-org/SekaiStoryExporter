//! # pjsk-scenario — 剧本解析
//!
//! ## 职责
//! - `ScenarioData` → IR
//! - 枚举与字段的版本适配（数据来自 `docs/reverse/versions/<ver>/constants.yaml`，经 xtask 代码生成）
//! - 全量剧本统计（覆盖率报告，见 `docs/architecture.md` 前三件事之三）
//!
//! ## 不负责
//! - 时间轴计算、分支拍平
//!
//! ## 允许依赖
//! `pjsk-core`、`pjsk-ir`、`pjsk-assets`。
