# 架构与路线

## 数据流

```
[资产层]   Web Assets Source / 本地 → manifest + CAS 缓存
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
pjskChatGenerator/
├── Cargo.toml               workspace（13 crate + xtask）
├── clippy.toml              ⭐ 确定性规约的编译期防线
├── rust-toolchain.toml      工具链钉版本 + 双平台 target
│
├── docs/
│   ├── decisions.md              ⭐ 18 项架构决策，单一事实来源
│   ├── architecture.md           本文件
│   ├── risks.md                  已知风险登记（R1 Live2D 语义偏差…）
│   ├── testing.md                五层测试策略
│   ├── versioning.md             版本策略 + 游戏更新适配 checklist
│   ├── conventions/
│   │   ├── determinism.md        确定性规约（D 级自动 / R 级人工）
│   │   └── language.md           语言规约（代码英文 / 文档中文）
│   ├── spec/
│   │   ├── tolerance.md          容差规范 v1（T1 / T2）
│   │   ├── coordinate-systems.md 7 套坐标系 + 时间基规约
│   │   ├── glossary.md           术语表
│   │   ├── ir.md                 ☐ 待设计
│   │   └── param-table.md        ☐ 待设计
│   ├── reverse/
│   │   ├── open-questions.md     19 项待确认事实（🔴🟡🟢）
│   │   ├── workflow.md           逆向工作流 + provenance 规约
│   │   ├── versions/<region>-<ver>/constants.yaml   ☐ 逆向产出，代码生成源
│   │   └── notes/                调查笔记（自由格式）
│   └── adr/                      单条决策详述
│
├── tools/                   伴生工具，非 Rust
│   ├── dumper/              游戏内 hook：UI 层次 / 参数 / atlas / 原始帧
│   ├── fetch/               Web Assets Source 抓取 + manifest
│   ├── oracle/              .NET 跑反编译 C#，产出 L1 fixture
│   └── stats/               全量剧本统计
│
├── crates/                  见下方依赖约束
└── xtask/                   constants.yaml → .rs 代码生成
```

### crate 依赖约束

```
pjsk-core            ← 无 pjsk-* 依赖（坐标系 newtype / det_math / 时间基）
pjsk-ir              ← core
pjsk-assets          ← core
pjsk-scenario        ← core, ir, assets
pjsk-timeline        ← core, ir, assets
pjsk-live2d          ← core, assets          （唯一允许 unsafe 的 crate）
pjsk-ugui            ← core, assets
pjsk-text            ← core, assets, ugui
pjsk-bake            ← 以上大部分          （Pass 1）
pjsk-render          ← core, assets, live2d, ugui, text   （Pass 2）
pjsk-export          ← core, render
pjsk-fidelity        ← core
pjsk-cli             ← 全部
```

> **铁律**：`pjsk-render` 不得依赖 `pjsk-timeline` 或 `pjsk-scenario`。
> 渲染只认参数表。这条依赖约束是 Pass 2 无状态性的结构保证，破坏它等于破坏 Q14 架构。


---

## 路线（Q18=C：正确性优先）

| 阶段 | 内容 | 完成判据 |
|---|---|---|
| **P0** | 逆向确认 + 工具链 | 🔴 六项全部关闭；UI dumper / 参数 dumper / atlas dumper / oracle 可跑 |
| **P1** | 数据层 | 资产抓取可用；ScenarioData → IR；**全量剧本统计报告产出** |
| **P2** | 时间轴 + 参数表 | Pass 1 可产出参数表，并与 hook 游戏 dump 的参数逐数值比对通过 |
| **P3** | 最小渲染 + 导出打通 | UI 还很丑，但能出音画同步的 mp4 + 帧哈希 |
| **P4** | fidelity harness | 形态分类器可用；T1/T2 报告自动产出 |
| **P5** | UGUI 布局 + 文本 | 布局单测全绿；文本在参考分辨率下过 T2 |
| **P6** | 特效覆盖 | 按统计报告的出现频次排序补齐 |
| **P7** | 预览器 | 消费同一份参数表 |

**P3 刻意排在 UI 之前**：导出链路的坑（确定性、音画同步、色彩空间）越早暴露越好，不要等 UI 做完才发现要重构。

---

## 接下来的前三件事

1. **确认色彩空间**（`reverse/open-questions.md` #1）——最便宜、错了代价最大，查一下 PlayerSettings 就有。
2. **写 UI 层次 dumper**——Q9=D 的前置。在 N 个分辨率 × M 个宽高比下遍历所有 Canvas 下的 RectTransform，输出
   `path / anchorMin / anchorMax / offsetMin / offsetMax / pivot / sizeDelta / localScale / rotation / active / 组件列表 / 最终屏幕 rect`。
   这一份产出同时是：布局参数的唯一可信来源、布局求值器的单测 fixture、游戏更新后的回归网。
3. **全量剧本统计**——让 Q10=A 的取舍有数据：
   - 每种 `SnippetAction` 出现次数
   - 每种 `SpecialEffectType` 出现次数 + 覆盖多少剧本
   - 含 `Selectable` 的剧本占比
   - 引用 3D / 未支持指令的剧本占比
   - 去重后的 Live2D 模型 / 动作 / 表情全集（决定资产下载量）

   预期结论形态：少数几种特效占绝大多数出现次数——那就能以极低成本把覆盖率从 A 拉到接近 C。
