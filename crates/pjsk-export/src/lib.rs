//! # pjsk-export — 导出与编码
//!
//! ## 职责
//! - 异步回读流水线（三缓冲 staging；Windows/PCIe 上是性能关键，macOS 统一内存上不是）
//! - 输出形态（Q15，全部为参数开关）：mp4 直出 / 无损帧序列 / BLAKE3 帧哈希 / 分层 / draw call dump（窄条件触发）
//! - 离线混音：时间轴 → ffmpeg `filter_complex` → 与视频流 mux
//!
//! ## 验收载体
//! 逐帧 BLAKE3 哈希，**不在 mp4 层面比对**（见 `docs/spec/tolerance.md`）。
