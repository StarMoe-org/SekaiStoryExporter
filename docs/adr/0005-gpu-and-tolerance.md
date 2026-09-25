# ADR-0005: GPU 后端、平台与保真度容差

- **状态**：accepted
- **日期**：2026-09-22

## 背景

目标是与游戏画面对应，并在不同平台上得到一致的结果。GPU 渲染在不同厂商、驱动之间无法做到逐位一致。

## 决策

- 渲染使用 wgpu（macOS 为 Metal，Windows 为 DX12，Linux 为 Vulkan）。
- 放弃逐位一致，改为可度量的容差（[`spec/tolerance.md`](../spec/tolerance.md)）：T1 为跨平台一致性，要求严格；T2 为与游戏的一致性，要求较松。判定时主要看差异的形态，而不是幅度。
- 为此执行确定性规约（[`conventions/determinism.md`](../conventions/determinism.md)）：其中 D 级规则由 `clippy.toml` 在编译期强制，R 级规则在评审中检查。
- 支持的平台：macOS（Apple Silicon）、Windows x64、Linux x64。

## 后果

- 正面：可以利用 GPU 性能，同时让跨平台差异保持可控、可度量。
- 负面：需要维护确定性规约，例如禁用某些超越函数、固定 draw 顺序、不用 `HashMap`。

## 复审条件

出现无法用容差解释的跨平台差异时。
