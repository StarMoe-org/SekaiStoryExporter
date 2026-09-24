# SekaiStoryExporter

**SekaiStoryExporter**（缩写 `sse`）：还原 Project Sekai 的 Live2D 剧情演出（AVG），并导出为视频。

| | |
|---|---|
| 语言 | Rust |
| 渲染 | wgpu（macOS / Metal，Windows / DX12） |
| 平台 | macOS (Apple Silicon)、Windows (NVIDIA) |
| 状态 / Status | **早期实现**：第一话可导出自动播放视频（近似项见导出报告） / early: episode 1 exports to video |
| 许可 / Licence | AGPL-3.0-or-later + [Cubism Core 链接例外](LICENSE-EXCEPTION) |

## 支持矩阵

| 项目版本 | 游戏 region | 游戏版本 | 状态 |
|---|---|---|---|
| 0.0.0 | cn | 6.4.0 (Unity 2022.3.62f3) | 逆向阶段：剧本层已还原，见 [`docs/reverse/versions/cn-6.4.0/`](docs/reverse/versions/cn-6.4.0/) |

## 文档导航

新人按顺序读前三份：

| | 文档 | 内容 |
|---|---|---|
| 1 | [`docs/decisions.md`](docs/decisions.md) | ⭐ 架构决策（Q1–Q39），**单一事实来源** |
| 2 | [`docs/spec/glossary.md`](docs/spec/glossary.md) | 术语表，写代码前必读 |
| 3 | [`docs/spec/coordinate-systems.md`](docs/spec/coordinate-systems.md) | 7 套坐标系 + 时间基规约，本项目最高频 bug 来源 |
| | [`docs/architecture.md`](docs/architecture.md) | 数据流、仓库结构、crate 依赖约束、路线 |
| | [`docs/conventions/determinism.md`](docs/conventions/determinism.md) | 确定性规约（D 级自动检查 / R 级人工评审） |
| | [`docs/spec/tolerance.md`](docs/spec/tolerance.md) | 保真度容差规范（T1 跨平台 / T2 对游戏） |
| | [`docs/testing.md`](docs/testing.md) | 五层测试策略 |
| | [`docs/reverse/workflow.md`](docs/reverse/workflow.md) | 逆向工作流与 provenance 规约 |
| | [`docs/reverse/work-order.md`](docs/reverse/work-order.md) | ⭐ 逆向工作单：需要从游戏中取得的内容 |
| | [`docs/reverse/open-questions.md`](docs/reverse/open-questions.md) | 待确认事实清单 |
| | [`docs/reverse/versions/cn-6.4.0/`](docs/reverse/versions/cn-6.4.0/) | ⭐ **逆向结论**：枚举、常量、坐标映射、状态机、特效语义 |
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

## 使用（第一话导出）

```sh
# 1. 用 SekaiStoryRipper 导出剧情资产（需自备解密密钥）
ripper --out <ripper-out> rip unit:school-refusal-story-chapter/1

# 2. 准备（均由你自行获取，不随本仓库分发）
#    - Live2D Cubism SDK for Native（接受其条款），设置 SSE_CUBISM_CORE_DIR=<sdk>/Core
#    - --ui 目录：SourceHanSansSC-Medium.otf / -Bold.otf（思源黑体，SIL OFL）；
#      从自己的客户端（ipa 的 data.unity3d）导出的 UI sprite，按 sprite 名存为 PNG：
#      bg_story_adv / bg_base_half_r8_wh / bg_base_round_h48_wh / icon_triangle_h22_wh /
#      btn_circle_h80_wh / icon_menu_story_wh（缺哪张就不画哪个元素，并写入报告）；
#      没有 bg_story_adv 时退回 Dialogue_Background.png 叠层
export SSE_CUBISM_CORE_DIR=/path/to/CubismSdkForNative/Core
cargo build --release -p sse-cli

# 3. 导出（需要 ffmpeg）
./target/release/sse --library <ripper-out> export unit:school-refusal-story-chapter/1 \
    -o out/ep1.mp4 --ui <ui-dir>
# 单帧：sse ... render <selector> --frame 6500 -o f.png --ui <ui-dir>
# 调试：sse ... inspect <selector>（IR）/ timeline <selector> / bake <selector>
```

导出旁会生成 `*.report.txt`，列出本次输出中所有近似与未支持项。

## 资产

**本仓库不包含、也不分发任何游戏资产。**

Live2D 模型、动作、字体、语音、BGM、背景图等全部版权归 SEGA / Colorful Palette 所有。
运行本工具需要你自行提供资产：用伴生工具 [SekaiStoryRipper](https://github.com/StarMoe-org/SekaiStoryRipper)
从游戏 CDN 下载并解包（需自备解密密钥），sse 只读取它的输出目录（`library/` + `episodes/`）。
sse 本身不含任何下载或解密代码。

缺少资产时工具会明确报错并列出缺失清单，不会静默降级。

## 法律边界

本项目是资产查看与研究工具，代码开源，资产不分发。请勿使用本工具产出的内容进行商业用途或再分发。

Live2D Cubism SDK 的使用与分发受 Live2D 自身条款约束，本仓库不包含其二进制，需用户自行获取并同意其条款。许可证事项见 [ADR-0002](docs/adr/0002-license.md)（AGPL-3.0-or-later + Cubism Core 链接例外）。


---

## English summary

**SekaiStoryExporter** (`sse`) is a tool that reproduces Project Sekai's Live2D story scenes (visual-novel style) and
exports them to video. Written in Rust, rendering through wgpu (Metal on macOS,
DX12 on Windows).

**This repository ships no game assets.** Live2D models, motions, fonts, voice lines,
BGM and backgrounds all remain the property of SEGA / Colorful Palette. You must
supply them yourself with the companion tool
[SekaiStoryRipper](https://github.com/StarMoe-org/SekaiStoryRipper), which downloads and
unpacks them (you provide the decryption key); sse only reads its output and contains
no download or decryption code. The tool fails loudly with a list of missing assets rather
than degrading silently.

The Live2D Cubism SDK is **not** bundled. You must obtain it from Live2D Inc. and
accept their terms. The project is AGPL-3.0-or-later with an additional permission
allowing linking against Cubism Core -- see [`LICENSE-EXCEPTION`](LICENSE-EXCEPTION)
and [ADR-0002](docs/adr/0002-license.md).

Design documents live under `docs/` and are written in Chinese; code and commit
messages are in English. **Pull requests and issues are welcome in either language.** See [`docs/conventions/language.md`](docs/conventions/language.md).
