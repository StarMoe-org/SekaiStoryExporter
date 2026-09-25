# Changelog

格式参考 [Keep a Changelog](https://keepachangelog.com/)。

**本项目的特殊约定**：任何会改变输出像素的变更，必须在此标注并注明影响范围。
**Project-specific rule**: any change that alters output pixels must be recorded here
together with its impact.

## 未发布

### 新增 / Added

- **S3**：`--library s3://bucket/prefix` 直接读取 SekaiStoryRipper 发布到 S3（或 MinIO、R2 等兼容服务）的 library，按 episode 只同步所需文件到本地缓存（`--cache-dir` / `SSE_CACHE_DIR`）；`-o s3://bucket/key` 渲染完成后上传视频与报告。凭据只从 `AWS_*` 环境变量读取。见 ADR-0015。无像素影响（与本地 library 逐字节相同）。
  `--library` and `-o` accept `s3://` URLs; only the files an episode needs are fetched.

### 修复 / Fixed

- **对话框文字描边宽度**：描边层（`WordsOutline` / `NameOutline`，shader `Sekai/TextMeshPro/Mobile/Distance Field`）
  的外扩量按客户端 shader 代码与材质参数计算：`(_FaceDilate 0.5 + _OutlineWidth 0.5) × ratioA / 2` 个 SDF 单位，
  1 SDF 单位 = 12 个图集像素（由游戏自带的 SDF 图集实测），即 4.21 × 字号 / 35 像素。此前只计入了 `_FaceDilate`，
  且用的是拟合值，描边只有游戏的约 40%。描边改用精确的欧氏距离变换生成。
  **像素影响**：CN 与 JP 所有对话框文字、名字的描边变宽（44 号字 1080p 下约 2.3 px → 5.8 px），与游戏录像对照一致。
  Talk window text outlines now follow the client's shader: about 2.5× wider than before.
- **ShakeWindow（特效 6 / 26）只抖文字**：`TalkWindow.windowRectTransform` 在 prefab 里指向
  `Window/ContentRoot/Content/Text`，其下只有名字与正文（及其描边层）。此前把对话框底板、名字横条和 AUTO 标签也一起抖了。
  **像素影响**：ShakeWindow 期间对话框底板、名字横条、AUTO 标签保持不动。
  ShakeWindow now moves only the name and words, as the prefab does.
- **Layout 附带的动作与表情**：`SnippetActionCharacterLayout` 在按类型分支之前，先对任意类型执行 `MotionName`（换身体动作）与 `FacialName`（换表情）。
  此前只在出场（Type 2）时使用，移动（1）、退场（3）、层级调整（6）所带的动作与表情都被丢弃，角色在这些时刻僵住不动。IR v3：`Layout.motion` / `Layout.facial`。
  **像素影响**：几乎所有剧集中，移动、退场、层级调整时的动作与表情现在会播放（本地日服每话 7–43 处）；节奏不变（CN 第一章 440.6 s、日服 217 第 3 话 697.2 s 均不变）。
  Layout snippets of every type now apply their motion and facial, as the game does before branching on the type.

## 0.1.0（2026-09-25）

首个公开版本。把 SekaiStoryRipper 导出的剧情（CN 6.4.0 / JP 6.8.1）渲染成与游戏一致的自动播放视频。
First public release: renders Project Sekai Live2D stories ripped by SekaiStoryRipper
(CN 6.4.0 / JP 6.8.1) into auto-play videos that match the game.

### 导出链路 / Pipeline

- `sse-assets`（读 Ripper 输出）→ `sse-scenario`（剧本 → IR）→ `sse-timeline`（60 fps 逐帧复刻协程调度）
  → `sse-bake`（Unity 语义的动作 / 表情线性混合、眨眼、口型、呼吸、物理 → 参数表）→ `sse-render`（wgpu）
  → `sse-export`（离线混音 + ffmpeg H.264）。
- CLI：`sse inspect | timeline | bake | render | export`；`--output-size WxH` 先按设备分辨率渲染再缩放编码；
  `--player-name` 替换 `{{playerName}}`；`--game cn|jp` 覆盖按 Ripper 输出自动判断的区服。
- 每次导出旁生成 `*.report.txt`，列出本次输出中的全部近似与未支持项。

### 画面 / Visuals

- 角色：每角色 2304×1536 RT + 二次乘 alpha 合成、遮罩；纵向位置与游戏的站位一致。
- 对话框按游戏的 `TalkWindow` 重建：底板渐变、名字横条、AUTO 标签与闪烁三角、右上菜单按钮；
  开合、打字机节奏、台词内嵌动作的时刻与游戏一致。
- 全屏文字（黑边、逐字渐显、阴影）、Telop 条、地点栏。
- 背景交叉淡入、`ColorFader`、相机模糊（点采样降采样，模糊大小随渲染分辨率变化）、相机色调、片尾影片。
- 特效：ShakeScreen / ShakeWindow（DOTween Shake 算法）、SideFade、Sekai 转场粒子、剧情特效 prefab
  （通用 Animator 与 Unity 粒子系统；逐粒子位置与游戏不同，随机数不可复现）。
- 文字：用客户端自带的源字体光栅化，排版参数（字号、行距、自动缩放、字距、描边层）与游戏一致。

### 节奏 / Pacing

- 自动翻页等待一次性身体动作、语音结束以 CRI 报告的播放结束为准（iOS Sonic Sync 比最后一个采样晚 50 ms）、
  退场前置等待、各类淡变的帧数，均按游戏行为实现。
- 对照同一话在游戏中的录像：65 个语音间隔的平均误差 0.023 s，整话累计漂移 −0.89 s。

### 项目 / Project

- 公开到 GitHub（[StarMoe-org/SekaiStoryExporter](https://github.com/StarMoe-org/SekaiStoryExporter)）。
- **Cubism Core 改为运行时加载**（`SSE_CUBISM_CORE` / `SSE_CUBISM_CORE_DIR` / 可执行文件旁 / 系统路径）：
  编译和测试不再需要 Cubism SDK，发布包不含 Core（ADR-0003）。无像素影响（与静态链接逐字节相同）。
- **UI 套件导出脚本** `tools/ui-kit/extract.py`：从用户自己的客户端（`.ipa`、`.app`、`Data` 或 `data.unity3d`）
  导出对话框 sprite、转场贴图、字体和转场粒子参数（ADR-0006）。
- **转场粒子参数改为运行时读取**（UI 套件里的 `fx_transition_scenario.json`，`sse-fx` v1），源码中不再内嵌游戏数据。
  无像素影响（与原先编译进程序的参数逐值相同）。
- **像素影响**：用导出脚本生成的 UI 套件时，
  - CN 使用客户端自带的字体（思源黑体的另一个版本），文字边缘与之前下载的思源黑体略有差异；
  - 菜单图标改用游戏剧情界面实际引用的那一张（ScenarioAtlas），与之前 CN 套件里的 CommonAtlas 版本有细微差别。
- 架构决策整理为 ADR（`docs/adr/`）；CI 同时提供 GitHub Actions 与 Gitea Actions，在 macOS、Windows、Linux 上构建和测试；
  推送 `v*` tag 时自动发布各平台二进制。
- 依赖 `ripper-format` v0.2.0。
