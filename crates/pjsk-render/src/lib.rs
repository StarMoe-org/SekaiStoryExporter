//! # pjsk-render — Pass 2：无状态渲染
//!
//! ## 职责
//! - `render(frame_n)`：输入参数表的一帧，输出像素。**必须无状态**
//! - wgpu 管线、SSAA、分层输出（Q15，默认关闭）
//! - 着色器：TMP_SDF、Cubism、各特效
//!
//! ## 铁律
//! > **不得依赖 `pjsk-timeline` 或 `pjsk-scenario`。**
//! > 渲染只认参数表。这条依赖约束是 Pass 2 无状态性的结构保证，破坏它等于破坏 Q14 架构。
//!
//! ## 确定性
//! 本 crate 是确定性规约的主战场，见 `docs/conventions/determinism.md`。
