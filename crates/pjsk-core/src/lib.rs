//! # pjsk-core — 基础类型与确定性设施
//!
//! 跨全项目共享的最底层设施。**不含任何业务逻辑。**
//!
//! ## 职责
//! - 坐标系与单位的 newtype（见 `docs/spec/coordinate-systems.md`）
//! - `det_math`：跨平台逐位一致的超越函数（基于 `libm` crate，不用 `std`）
//! - `fs::read_dir_sorted` 等确定性工具函数
//! - 时间基类型：`FrameNo`、`Seconds`、`TimeBase`
//!
//! ## 不负责
//! - 任何与 PJSK 业务相关的概念
//!
//! ## 允许依赖
//! 无 `pjsk-*` 依赖。这是依赖图的叶子。
