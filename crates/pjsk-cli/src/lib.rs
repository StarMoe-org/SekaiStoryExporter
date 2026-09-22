//! # pjsk-cli — 命令行入口
//!
//! ## 职责
//! - 子命令编排：fetch / stats / compile / bake / render / export / diff
//! - `OutputConfig` 的壳（Q15：输出形态是结构体，CLI 只是它的一层壳）
//! - 预览器（P7）也在此挂载，与导出器共用同一份参数表
