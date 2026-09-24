# 架构与路线

## 数据流

```
[资产层]   SekaiStoryRipper 输出（library/ + episodes/，Q25）→ 只读加载 + 校验
[解析层]   ScenarioData → IR (JSON)
[编译层]   IR + 语音时长 + 逆向常量 → 绝对时间轴
[Pass 1]   顺序空跑时间轴，只更新状态 → 参数表（纯数据，逐帧全量）
[Pass 2]   参数表 → 无状态渲染 render(frame_n) → 帧 / 哈希 / 分层
[导出层]   帧序列 或 ffmpeg 管道 + 离线混音 → mp4
```

### 为什么是两阶段（Q14=B）

Live2D 的 motion fade 与 physics 是累积状态，不能任意 seek。但**状态更新与渲染可以分离，且前者极快**（纯 CPU 小计算，不碰 GPU）。

于是把状态跑成一张参数表，**一个中间层同时解决四件事**：

| | |
|---|---|
| 可观测性 | 可与 hook 游戏 dump 出的参数逐数值比对，把四维 bug 空间切成两个二维 |
| 确定性 | Pass 2 完全无状态 |
| 并行 | 每帧独立，可并行 / 分布式 |
| seek | 预览器拖时间轴瞬间跳转 |

数据量：一个模型约 200 参数 × 4B ≈ 800B/帧；三模型加 UI/背景/特效状态约 3KB/帧。60fps 十分钟约 100MB 未压缩，帧间 delta + zstd 后通常剩几 MB。

`--fused` 开关把 Pass1/Pass2 融进同一循环，即退化为单阶段顺序渲染（Q14 的 fallback，免费）。

---

## 仓库结构

```
SekaiStoryExporter/
├── Cargo.toml               workspace（14 crate + xtask）
├── clippy.toml              ⭐ 确定性规约的编译期防线
├── rust-toolchain.toml      工具链钉版本 + 双平台 target
│
├── docs/
│   ├── decisions.md              ⭐ 架构决策（Q1–Q39），单一事实来源
│   ├── architecture.md           本文件
│   ├── risks.md                  已知风险登记（R1 Live2D 语义偏差…）
│   ├── testing.md                五层测试策略
│   ├── versioning.md             版本策略 + 游戏更新适配 checklist
│   ├── conventions/
│   │   ├── determinism.md        确定性规约（D 级自动 / R 级人工）
│   │   └── language.md           语言规约（代码英文 / PR·Issue 双语 / 文档中文）
│   ├── spec/
│   │   ├── tolerance.md          容差规范 v1（T1 / T2）
│   │   ├── coordinate-systems.md 7 套坐标系 + 时间基规约
│   │   ├── glossary.md           术语表
│   │   ├── ir.md                 IR schema（M1）
│   │   └── param-table.md        参数表格式（M1）
│   ├── reverse/
│   │   ├── open-questions.md     待确认事实清单（🔴🟡🟢）
│   │   ├── work-order.md         ⭐ 逆向工作单（RE-01…13 / RT-01…05）
│   │   ├── workflow.md           逆向工作流 + provenance 规约
│   │   ├── versions/<region>-<ver>/   逆向结论（constants.yaml 为代码生成源）
│   │   └── notes/                调查笔记（自由格式）
│   └── adr/                      单条决策详述
│
├── tools/                   伴生工具，非 Rust
│   ├── dumper/              游戏内 hook：UI 层次 / 参数 / atlas / 原始帧
│   ├── oracle/              .NET 跑反编译 C#，产出 L1 fixture
│   └── stats/               全量剧本统计
│
├── crates/                  见下方依赖约束
└── xtask/                   constants.yaml → .rs 代码生成
```

### crate 依赖约束

```
sse-core            ← 无 sse-* 依赖（坐标系 newtype / det_math / 时间基）
sse-ir              ← core
sse-assets          ← core, ripper-format（外部，仅 serde 类型，Q26）
sse-scenario        ← core, ir, assets
sse-timeline        ← core, ir, assets
sse-live2d          ← core, assets          （唯一允许 unsafe 的 crate）
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
> 渲染只认参数表。这条依赖约束是 Pass 2 无状态性的结构保证，破坏它等于破坏 Q14 架构。


---

## 路线（Q18=C：正确性优先）

| 阶段 | 内容 | 完成判据 |
|---|---|---|
| **P0** | 逆向确认 + 工具链 | 🔴 六项全部关闭；UI dumper / 参数 dumper / atlas dumper / oracle 可跑 |
| **P1** | 数据层 | 读取 Ripper 输出可用；ScenarioData → IR；**全量剧本统计报告产出** |
| **P2** | 时间轴 + 参数表 | Pass 1 可产出参数表，并与 hook 游戏 dump 的参数逐数值比对通过 |
| **P3** | 最小渲染 + 导出打通 | UI 还很丑，但能出音画同步的 mp4 + 帧哈希 |
| **P4** | fidelity harness | 形态分类器可用；T1/T2 报告自动产出 |
| **P5** | UGUI 布局 + 文本 | 布局单测全绿；文本在参考分辨率下过 T2 |
| **P6** | 特效覆盖 | 按统计报告的出现频次排序补齐 |
| **P7** | 预览器 | 消费同一份参数表 |

**P3 刻意排在 UI 之前**：导出链路的坑（确定性、音画同步、色彩空间）越早暴露越好，不要等 UI 做完才发现要重构。

---

## 接下来（Q31）

早期的「前三件事」（色彩空间、UI dumper、全量剧本统计）中，色彩空间与剧本统计已完成，UI dumper 归入 RT-01。

**M1 · CPU 确定性链路**（无 GPU）

1. `spec/ir.md`、`spec/param-table.md` 定稿
2. `sse-assets`：读取 `ripper-episode` / `sse-motion`，拒绝未知版本，缺资产列清单并提示 `ripper rip <selector>`
3. `sse-scenario`：`ScenarioSceneData` → IR
4. `sse-timeline`：IR + 语音时长 + 常量 → 绝对时间轴
5. `sse-bake`：Pass 1，Unity 语义的动作求值与线性混合（Q24）、眨眼、口型（Q27）→ 参数表
6. `sse inspect` / `sse params` + golden test；RT-02 就绪后做参数级比对

**M2 · 静态首帧**：Cubism Core FFI + wgpu 单角色渲染进 2304×1536 RT，输出 PNG。
