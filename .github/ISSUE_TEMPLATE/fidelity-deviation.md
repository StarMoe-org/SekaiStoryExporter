---
name: 保真度偏差 / Fidelity deviation
about: 渲染结果与游戏（T2）或与另一平台（T1）不符 / Output differs from the game (T2) or across platforms (T1)
labels: fidelity
---

<!-- 中英皆可 / Chinese or English, whichever is clearer. -->

## 类型 / Type

- [ ] **T1** 跨平台 / cross-platform (macOS renderer vs Windows renderer)
- [ ] **T2** 对游戏 / against the game (renderer vs the same scene in the game)

## 复现坐标 / Reproduction coordinates

| | |
|---|---|
| 剧本 ID / scenario id | |
| 帧号 / frame no | |
| 目标分辨率与宽高比 / target resolution & aspect | |
| SSAA 倍率 / factor | |
| fps | |
| 游戏版本与 region / game version & region | |
| 平台与 GPU / platform & GPU | |

## 差异形态 / Difference shape

> 形态比幅度重要 / shape matters more than magnitude. See `docs/spec/tolerance.md`.

- [ ] 边缘带 / edge band (within ±1px of a geometric edge) — 可能是可接受的浮点差异
- [ ] 区块 / block (large constant offset) — **必为 bug**: colour space / blend / premultiplied alpha
- [ ] 孤点 / isolated point — **必为 bug**: draw order / masking
- [ ] 全局偏移 / global offset

## 量化 / Measurements

| 指标 / metric | 实测 / actual | 阈值 / threshold |
|---|---|---|
| 最大单通道差 / max channel diff | | |
| 差异像素占比 / differing pixel ratio | | |
| SSIM | | |

## 分层归因 / Per-layer attribution

<!-- 若开启了分层输出（--layers） / if layered output was enabled -->

## 参数级比对 / Parameter-level comparison

<!-- 这一步决定问题在渲染层还是状态层 / this decides render layer vs state layer -->

- [ ] 参数表一致 → 问题在渲染层 / parameter table matches → issue is in the render layer
- [ ] 参数表不一致 → 问题在动作/物理/状态机 / mismatch → issue is in motion/physics/state machine
- [ ] 未做 / not done
