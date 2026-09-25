# ADR-0007: Live2D 语义：Unity 侧行为

- **状态**：accepted（取代早期「照搬 Cubism Native Framework 语义」的决策）
- **日期**：2026-09-24

## 背景

游戏在 Unity 中播放 Live2D：动作是 AnimationClip，层间混合由 Unity 完成，模型变形仍由 Cubism Core 计算。资产里没有 motion3.json，第三方转换出的 motion3 已知有损，把零切线三次段写成了线性。

## 决策

- 动作曲线来自 Ripper 的 `sse-motion`（无损保留 StreamedClip 系数），按 Unity 的求值方式计算；层间线性交叉淡化。Native Framework 的动作与淡入淡出逻辑不移植。
- 物理、遮罩、眨眼按游戏行为复刻。
- 口型同步：有语音时由音量（RMS）驱动，无语音时按固定序列随文字推进。
- 动作名到 clip 的解析以 episode 索引为准，sse 只做一致性校验，不重新实现解析规则；索引里缺的动作按游戏行为保持上一动作。
- sse 内部不得用剧本 JSON 内的 `ScenarioId` 反查资产，一律使用索引给出的路径。

## 后果

- 正面：动作求值只有一种实现，而且输入无损。
- 负面：与 Live2D 官方 SDK 的行为不同，不能直接复用其工具链验证。

## 复审条件

游戏改用 motion3 或更换 Live2D 集成方式时。
