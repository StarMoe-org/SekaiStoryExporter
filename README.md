# pjskChatGenerator

还原 Project Sekai 的 Live2D 剧情演出（AVG），并导出为视频。

| | |
|---|---|
| 语言 | Rust |
| 渲染 | wgpu（macOS / Metal，Windows / DX12） |
| 平台 | macOS (Apple Silicon)、Windows (NVIDIA) |
| 状态 / Status | **设计阶段**，尚无实现代码 / design stage, no implementation yet |
| 许可 / Licence | AGPL-3.0-or-later + [Cubism Core 链接例外](LICENSE-EXCEPTION) |

## 支持矩阵

| 项目版本 | 游戏 region | 游戏版本 | 状态 |
|---|---|---|---|
| 0.0.0 | — | — | 未开始 |

## 文档导航

新人按顺序读前三份：

| | 文档 | 内容 |
|---|---|---|
| 1 | [`docs/decisions.md`](docs/decisions.md) | ⭐ 18 项架构决策，**单一事实来源** |
| 2 | [`docs/spec/glossary.md`](docs/spec/glossary.md) | 术语表，写代码前必读 |
| 3 | [`docs/spec/coordinate-systems.md`](docs/spec/coordinate-systems.md) | 7 套坐标系 + 时间基规约，本项目最高频 bug 来源 |
| | [`docs/architecture.md`](docs/architecture.md) | 数据流、仓库结构、crate 依赖约束、路线 |
| | [`docs/conventions/determinism.md`](docs/conventions/determinism.md) | 确定性规约（D 级自动检查 / R 级人工评审） |
| | [`docs/spec/tolerance.md`](docs/spec/tolerance.md) | 保真度容差规范（T1 跨平台 / T2 对游戏） |
| | [`docs/testing.md`](docs/testing.md) | 五层测试策略 |
| | [`docs/reverse/workflow.md`](docs/reverse/workflow.md) | 逆向工作流与 provenance 规约 |
| | [`docs/reverse/work-order.md`](docs/reverse/work-order.md) | ⭐ 逆向工作单：需要从游戏中取得的内容 |
| | [`docs/reverse/open-questions.md`](docs/reverse/open-questions.md) | 待确认事实清单 |
| | [`docs/versioning.md`](docs/versioning.md) | 版本策略与游戏更新适配 checklist |
| | [`docs/conventions/language.md`](docs/conventions/language.md) | 语言规约（代码与 commit 英文 / PR·Issue 中英皆可 / 设计文档中文） |
| | [`docs/risks.md`](docs/risks.md) | 已知风险登记 |
| | [`CONTRIBUTING.md`](CONTRIBUTING.md) | 协作流程与 PR 清单 |

## 环境

| 依赖 | 用途 | 必需 |
|---|---|---|
| Rust | 主工具链，版本由 `rust-toolchain.toml` 钉死（当前 1.98.1） | ✅ |
| Git LFS | 二进制 fixture，克隆后执行 `git lfs install` | ✅ |
| ffmpeg | 视频编码与混音 | ✅ |
| Live2D Cubism Core | Live2D 模型解析与变形（用户自行获取，见 ADR-0002） | ✅ |
| .NET | 生成 L1 oracle fixture | 开发期 |
| PlayCover + 游戏 | 采集 ground truth | 保真度工作 |

## 资产

**本仓库不包含、也不分发任何游戏资产。**

Live2D 模型、动作、字体、语音、BGM、背景图等全部版权归 SEGA / Colorful Palette 所有。
运行本工具需要你自行提供资产，来源二选一：

- 从 Web Assets Source 抓取（`tools/fetch`）
- 本地自备目录

缺少资产时工具会明确报错并列出缺失清单，不会静默降级。

## 法律边界

本项目是资产查看与研究工具，代码开源，资产不分发。请勿使用本工具产出的内容进行商业用途或再分发。

Live2D Cubism SDK 的使用与分发受 Live2D 自身条款约束，本仓库不包含其二进制，需用户自行获取并同意其条款。许可证事项见 [ADR-0002](docs/adr/0002-license.md)（**待定，未定前请勿对外发布**）。


---

## English summary

A tool that reproduces Project Sekai's Live2D story scenes (visual-novel style) and
exports them to video. Written in Rust, rendering through wgpu (Metal on macOS,
DX12 on Windows).

**This repository ships no game assets.** Live2D models, motions, fonts, voice lines,
BGM and backgrounds all remain the property of SEGA / Colorful Palette. You must
supply them yourself, either by fetching from a web assets source (`tools/fetch`) or
from a local directory. The tool fails loudly with a list of missing assets rather
than degrading silently.

The Live2D Cubism SDK is **not** bundled. You must obtain it from Live2D Inc. and
accept their terms. The project is AGPL-3.0-or-later with an additional permission
allowing linking against Cubism Core -- see [`LICENSE-EXCEPTION`](LICENSE-EXCEPTION)
and [ADR-0002](docs/adr/0002-license.md).

Design documents live under `docs/` and are written in Chinese; code and commit
messages are in English. **Pull requests and issues are welcome in either language.** See [`docs/conventions/language.md`](docs/conventions/language.md).
