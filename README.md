# SekaiStoryExporter

[![CI](https://github.com/StarMoe-org/SekaiStoryExporter/actions/workflows/ci.yml/badge.svg)](https://github.com/StarMoe-org/SekaiStoryExporter/actions/workflows/ci.yml)

**SekaiStoryExporter**（缩写 `sse`）：还原 Project Sekai 的 Live2D 剧情演出，并导出为与游戏一致的自动播放视频。

> English summary at the [end of this page](#english-summary).

| | |
|---|---|
| 语言 | Rust |
| 渲染 | wgpu（macOS / Metal，Windows / DX12，Linux / Vulkan） |
| 平台 | macOS arm64、Windows x64、Linux x64 |
| 许可 | AGPL-3.0-or-later + [Cubism Core 链接例外](LICENSE-EXCEPTION) |

## 支持矩阵

| 区服 | 游戏版本 | 状态 |
|---|---|---|
| CN | 6.4.0 | ✅ 主线、活动、卡面、特别篇 |
| JP | 6.8.1 | ✅ 主线、活动、卡面、特别篇 |

两个区服共用同一套实现，只有字体等素材随客户端不同。
导出旁会生成 `*.report.txt`，列出本次输出中的全部近似与未支持项（例如 3D MV）。

## 快速上手

需要准备：[ffmpeg](https://ffmpeg.org/)、[uv](https://docs.astral.sh/uv/)，以及你自己合法持有的游戏客户端（`.ipa`）。

**1. 安装 sse。** 从 [Releases](https://github.com/StarMoe-org/SekaiStoryExporter/releases) 下载对应平台的压缩包，
或者从源码构建（不需要 Cubism SDK）：

```bash
cargo build --release -p sse-cli      # 产物在 target/release/sse
```

**2. 准备 Live2D Cubism Core。** Core 是 Live2D 的闭源库，不随本项目分发。
下载 [Cubism SDK for Native](https://www.live2d.com/sdk/download/native/) 并接受其条款，然后任选一种方式：

```bash
export SSE_CUBISM_CORE_DIR=/path/to/CubismSdkForNative/Core    # SDK 的 Core 目录
# 或者：把 Core/dll/<平台>/ 下的动态库（libLive2DCubismCore.dylib / Live2DCubismCore.dll /
#       libLive2DCubismCore.so）放到 sse 可执行文件旁边，或用 SSE_CUBISM_CORE 指向它
```

**3. 用 SekaiStoryRipper 下载剧情资源。** 见 [SekaiStoryRipper](https://github.com/StarMoe-org/SekaiStoryRipper) 的快速上手。
sse 只通过 Ripper 的输出（本地目录或 S3 前缀）与它交换数据，要求 Ripper 版本与 sse 依赖的 `ripper-format` 一致（当前为 **v0.3.0**）。
打开 library 时会先核对 `ripper.lock.json` 里的格式版本，不一致时直接报错，并提示用对应版本重新导出：

```bash
ripper rip unit:school-refusal-story-chapter/1              # CN，结果在 out/
ripper --region jp rip event:185/1                          # 日服，结果在 out/jp/
```

**4. 从客户端导出 UI 套件。** 对话框贴图、字体、转场粒子和全息扫描线贴图随安装包分发，不在 CDN 上。
已有的旧套件缺 `holo.png` 时，全息角色只少扫描线的微弱闪烁，重新导出即可补上。
用与剧情同一区服的客户端导出：

```bash
uv run tools/ui-kit/extract.py <your-client.ipa> ui-cn
```

也可以传 `.app` 目录、其中的 `Data` 目录或 `data.unity3d`。脚本只在本地读取，不联网。

**5. 导出视频。**

```bash
sse --library out export unit:school-refusal-story-chapter/1 -o ep1.mp4 --ui ui-cn
```

## 更多用法

```bash
# 单帧 PNG
sse --library out render unit:school-refusal-story-chapter/1 --frame 6500 -o f.png --ui ui-cn

# 按 4K 渲染、1080p 编码（与 4K 设备上游戏的画面一致，包括随分辨率变化的模糊大小）
sse --library out export <selector> -o ep.mp4 --ui ui-cn --width 3840 --height 2160 --output-size 1920x1080

# 调试：IR / 时间轴 / 参数表
sse --library out inspect <selector>
sse --library out timeline <selector>
sse --library out bake <selector> [--frame N]
```

其他选项：`--player-name`（替换 `{{playerName}}`，默认「世界」的居民）、`--from` / `--to`（只导出一段帧）、
`--crf`、`--ffmpeg`、`--game cn|jp`（默认按 Ripper 输出里记录的区服）。`--library` 与 `--ui` 也可以用环境变量
`SSE_LIBRARY` / `SSE_UI_DIR` 指定。

## 使用 S3

library 和输出都可以放在 AWS S3，或 MinIO、Cloudflare R2 等兼容服务上：

```bash
export AWS_ACCESS_KEY_ID=...  AWS_SECRET_ACCESS_KEY=...
export AWS_ENDPOINT_URL=https://<account>.r2.cloudflarestorage.com   # 非 AWS 时设置；AWS 用 AWS_REGION

# library 由 SekaiStoryRipper 用 --out s3://my-bucket/sekai/jp 发布
sse --library s3://my-bucket/sekai/jp export event:185/1 -o s3://my-bucket/videos/event_185_01.mp4 --ui ui-jp
```

- 每一话只下载实际用到的文件，缓存在 `--cache-dir`（默认 `~/.cache/sse`）；远端 bundle 更新后会自动重新下载。
- `-o s3://…` 先渲染到缓存目录，完成后上传视频和报告。设计见 [ADR-0015](docs/adr/0015-s3.md)。

## 文档

| 文档 | 内容 |
|---|---|
| [`docs/adr/`](docs/adr/) | 架构决策记录 |
| [`docs/architecture.md`](docs/architecture.md) | 数据流、仓库结构、crate 依赖约束 |
| [`docs/spec/glossary.md`](docs/spec/glossary.md) | 术语表 |
| [`docs/spec/coordinate-systems.md`](docs/spec/coordinate-systems.md) | 坐标系与时间基，最高频 bug 来源 |
| [`docs/spec/ir.md`](docs/spec/ir.md) / [`param-table.md`](docs/spec/param-table.md) | IR 与参数表格式 |
| [`docs/spec/tolerance.md`](docs/spec/tolerance.md) | 保真度容差（T1 跨平台 / T2 对游戏） |
| [`docs/conventions/determinism.md`](docs/conventions/determinism.md) | 确定性规约 |
| [`docs/testing.md`](docs/testing.md) | 测试策略 |
| [`docs/versioning.md`](docs/versioning.md) | 版本策略与游戏更新适配 |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | 协作流程与 PR 清单 |

设计文档以中文为主；代码与 commit 为英文；PR 与 Issue 中英文皆可（[语言规约](docs/conventions/language.md)）。

## 资产与法律边界

- **本仓库和发布包不包含、也不分发任何游戏资产。** Live2D 模型、动作、字体、语音、BGM、背景、UI 贴图等的版权归
  SEGA / Colorful Palette / Craft Egg 所有。CDN 资源由 SekaiStoryRipper 下载，UI 套件由你从自己的客户端导出；
  sse 本身不含任何下载或解密代码。
- **Live2D Cubism Core 不随本项目分发**，需你自行取得并同意其条款；sse 在运行时加载它。
- 缺少资产时工具会明确报错或在导出报告中列出，不会静默降级。
- 本项目与 SEGA、Colorful Palette、Craft Egg 及 Live2D Inc. 没有任何关联，仅供个人研究使用。
  请勿将产出内容用于商业用途或再分发，并自行遵守游戏服务条款与所在地法律。
- 许可证事项见 [ADR-0002](docs/adr/0002-license.md)。依 AGPL 第 13 条，若将本工具作为网络服务提供，须向其用户提供对应源码。

---

## English summary

**SekaiStoryExporter** (`sse`) reproduces Project Sekai's Live2D story scenes and exports them
to auto-play videos that match the game. Written in Rust, rendering through wgpu (Metal, DX12,
Vulkan). CN 6.4.0 and JP 6.8.1 are supported at the same level.

Quick start:

1. Build (`cargo build --release -p sse-cli`) or download a release. No Cubism SDK is needed to build.
2. Download the Live2D Cubism SDK for Native yourself and point `SSE_CUBISM_CORE_DIR` at its
   `Core` directory (or put the Core shared library next to `sse`). Core is loaded at run time.
3. Rip an episode with [SekaiStoryRipper](https://github.com/StarMoe-org/SekaiStoryRipper) v0.3.0 (the
   release whose `ripper-format` sse is built with; sse checks the library's `ripper.lock.json`
   before reading anything and refuses other format versions).
4. Export the UI kit from a game client you own: `uv run tools/ui-kit/extract.py <client.ipa> ui`.
5. `sse --library out export <selector> -o ep.mp4 --ui ui`.

Both `--library` and `-o` also accept `s3://bucket/…` (AWS S3 or compatible stores such as MinIO and
R2; credentials from `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY`). Only the files an episode needs
are fetched into a local cache.

**This repository ships no game assets and no Live2D Cubism Core.** Every asset remains the
property of SEGA / Colorful Palette / Craft Egg; you supply them yourself. Not affiliated with
SEGA, Colorful Palette, Craft Egg or Live2D Inc. Licensed under AGPL-3.0-or-later with an
additional permission for linking with Cubism Core — see [`LICENSE-EXCEPTION`](LICENSE-EXCEPTION)
and [ADR-0002](docs/adr/0002-license.md).

Design documents live under `docs/` and are written in Chinese; code and commit messages are in
English. **Pull requests and issues are welcome in either language.**
