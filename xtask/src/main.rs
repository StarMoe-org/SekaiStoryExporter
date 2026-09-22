//! xtask — 构建期任务
//!
//! ## 职责
//! - `constants.yaml` → `.rs` 代码生成（逆向常量不手抄进代码，见 docs/decisions.md Q16 注记）
//! - fixture 校验、CI 辅助任务
//!
//! 用法：`cargo xtask <task>`（需在 .cargo/config.toml 配 alias）

fn main() {
    eprintln!("xtask: 尚未实现任何任务");
}
