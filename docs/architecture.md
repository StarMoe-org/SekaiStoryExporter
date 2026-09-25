# 架构与路线

## 数据流

```
[资产层]   SekaiStoryRipper 输出（library/ + episodes/）+ UI 套件（--ui）→ 只读加载 + 校验（ADR-0006）
[解析层]   ScenarioData → IR (JSON)
[编译层]   IR + 语音时长 + 游戏常量 → 绝对时间轴
[Pass 1]   顺序空跑时间轴，只更新状态 → 参数表（纯数据，逐帧全量）
[Pass 2]   参数表 → 无状态渲染 render(frame_n) → 帧 / 哈希 / 分层
[导出层]   帧序列 或 ffmpeg 管道 + 离线混音 → mp4
```

### 为什么是两阶段（ADR-0004）

Live2D 的 motion fade 与 physics 是累积状态，不能任意 seek。但**状态更新与渲染可以分离，且前者极快**（纯 CPU 小计算，不碰 GPU）。

于是把状态跑成一张参数表，**一个中间层同时解决四件事**：

| | |
|---|---|
| 可观测性 | 参数可以逐数值检查，把「曲线求值错了」和「渲染错了」分开 |
| 确定性 | Pass 2 完全无状态 |
| 并行 | 每帧独立，可并行 / 分布式 |
| seek | 预览器拖时间轴瞬间跳转 |

数据量：一个模型约 200 参数 × 4B ≈ 800B/帧；三模型加 UI/背景/特效状态约 3KB/帧。60fps 十分钟约 100MB 未压缩，帧间 delta + zstd 后通常剩几 MB。

`--fused` 开关把 Pass1/Pass2 融进同一循环，即退化为单阶段顺序渲染（免费的 fallback）。

---

## 仓库结构

```
SekaiStoryExporter/
├── Cargo.toml               workspace（14 crate + xtask）
├── clippy.toml              ⭐ 确定性规约的编译期防线
├── rust-toolchain.toml      工具链钉版本
│
├── docs/
│   ├── adr/                      ⭐ 架构决策记录
│   ├── architecture.md           本文件
│   ├── testing.md                测试策略
│   ├── versioning.md             版本策略 + 游戏更新适配 checklist
│   ├── conventions/
│   │   ├── determinism.md        确定性规约（D 级自动 / R 级人工）
│   │   └── language.md           语言规约（代码英文 / PR·Issue 双语 / 文档中文）
│   └── spec/
│       ├── tolerance.md          容差规范（T1 / T2）
│       ├── coordinate-systems.md 坐标系 + 时间基规约
│       ├── glossary.md           术语表
│       ├── ir.md                 IR schema
│       └── param-table.md        参数表格式
│
├── tools/                   伴生工具（Python，用 uv 运行）
│   ├── ui-kit/              从用户自己的客户端导出 UI 套件（--ui）
│   └── stats/               剧本语料统计
│
├── crates/                  见下方依赖约束
└── xtask/                   构建期辅助任务
```

### crate 依赖约束

```
sse-core            ← 无 sse-* 依赖（坐标系 newtype / det_math / 时间基）
sse-ir              ← core
sse-assets          ← core, ripper-format（外部，仅 serde 类型，ADR-0006）；远端 library 另用 reqwest + rusty-s3（ADR-0015）
sse-scenario        ← core, ir, assets
sse-timeline        ← core, ir, assets
sse-live2d          ← core, assets          （唯一允许 unsafe 的 crate；运行时加载 Cubism Core）
sse-ugui            ← core, assets
sse-text            ← core, assets, ugui
sse-params          ← core                 （参数表类型，Pass 1 / Pass 2 的接缝）
sse-bake            ← core, ir, assets, timeline, live2d, params   （Pass 1）
sse-render          ← core, assets, live2d, text, params   （Pass 2）
sse-export          ← core, assets, params, render
sse-fidelity        ← core
sse-cli             ← 全部
```

> **铁律**：`sse-render` 不得依赖 `sse-timeline` 或 `sse-scenario`。
> 渲染只认参数表。这条依赖约束是 Pass 2 无状态性的结构保证，破坏它等于破坏 ADR-0004。
