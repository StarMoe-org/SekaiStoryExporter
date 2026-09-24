# Changelog

格式参考 [Keep a Changelog](https://keepachangelog.com/)。

**本项目的特殊约定**：任何会改变输出像素的变更，必须在此标注并注明 fidelity 影响范围。
**Project-specific rule**: any change that alters output pixels must be recorded here
together with its fidelity impact.

## [Unreleased]

### 修复 / Fixed（2026-09-24，对标原生录制的节奏校准）
- 用 PlayCover 原生录制（第一话，442 s）的 65 个语音间隔校准 `sse-timeline`：平均误差 0.663 s → 0.051 s，
  最大 3.15 s → 0.14 s（按录制时游戏的实际帧率 54.5 fps 模拟；方法与逆向依据见
  `docs/reverse/notes/2026-09-24-native-capture-pacing.md`）
- 自动翻页等待所有在场角色的一次性身体动作播完（`CheckAutoModeTalkNext`）；无语音台词不经过该闸门
- FullScreenText：黑边开场（1.0 s + 0.5 s）、按 TMP `characterInfo` 数组逐槽渐显（0.125 s/槽）、语音后停留 1.0 s、末条淡出 1.0 s；
  语音改在 `text_start` 播放
- Sekai 转场完成 = 白色 `ColorFader` 延迟（In 0.25 s / Out 0.5 s）+ Duration，并补画白色淡变（粒子仍未画）
- 退场前置等待 0.15 s（滑出式为移动时长 − 0.1 s），出场/退场淡变多 1 帧（`WaitForSeconds(0)`）
- 每帧先恢复 `PlayCore` 再恢复片段协程（Unity 延迟调用队列顺序）
- IR v2：`LayoutOp::Hide { delay }`
- 新增 `sse timeline --sim-fps <f>`（仅用于对标降帧录像）
- **像素影响**：整集时长与各句出现时刻改变（第一话 397.95 s → 438.43 s，原生录制 442.3 s 含录制前后余量）；Sekai 转场出现白屏淡入淡出；退场晚 0.15 s 开始淡出
- Pacing calibrated against a native capture: mean interval error 0.663 s → 0.051 s.


### 新增 / Added（2026-09-24，第一话导出）
- 完整链路：`sse-assets`（读 Ripper 输出）→ `sse-scenario`（剧本 → IR v1）→ `sse-timeline`（60 fps 逐帧复刻协程调度）
  → `sse-bake`（Unity 语义动作/表情线性混合、眨眼事件、口型、呼吸、物理 → 参数表）→ `sse-render`（wgpu：背景 cover、
  每角色 2304×1536 RT + `UI/Default` 二次乘 alpha 合成、遮罩、色幕、模糊、相机色调、对话框与文字）→ `sse-export`（离线混音 + ffmpeg H.264）
- 新 crate `sse-params`（参数表类型）；Cubism Core FFI（`sse-live2d`，`SSE_CUBISM_CORE_DIR`）
- CLI：`sse inspect | timeline | bake | render | export`
- **像素影响**：首个渲染实现。已知近似（角色纵向锚定、第三方 UI 叠层、光栅化文字、遮罩分辨率等）逐条写入导出报告
- Added the full export pipeline; `unit:school-refusal-story-chapter/1` renders to a 1080p60 MP4.

### 变更 / Changed
- 项目改名为 **SekaiStoryExporter**（缩写 `sse`）：13 个 crate 由 `pjsk-*` 改为 `sse-*`，
  Rust 路径 `pjsk_core` → `sse_core`，CLI 子命令写作 `sse <cmd>`。无像素影响（决策 Q23）
- Renamed the project to **SekaiStoryExporter** (`sse`): crates `pjsk-*` → `sse-*`. No pixel impact.

### 新增 / Added
- 项目骨架：Cargo workspace（13 crate + xtask），仅含职责文档注释，无实现
- 确定性规约落地为 `clippy.toml` 的编译期 deny，**已实测 5 条规则全部触发**
- 工具链钉死 1.98.1 + 双平台 target
- 规约文档：确定性、坐标系与时间基、术语表、语言规约、逆向 provenance、版本策略、测试策略
- 协作设施：CONTRIBUTING（双语）、ADR 体系、PR 与 issue 模板
- 许可：AGPL-3.0-or-later + Live2D Cubism Core 链接例外
- Git LFS 管理二进制 fixture

### 决策 / Decisions
- Q19 许可证 = AGPL-3.0-or-later（+ 链接例外）
- Round 6（Q24–Q31）：Live2D 改为 Unity 语义（取代 Q8）；资产只来自 SekaiStoryRipper 输出，
  `ripper-format` 以 git tag 依赖；口型同步解挂；Q28 分辨率定义待 PlayCover 实测；
  M1 = CPU 确定性链路，M2 = 静态首帧。无像素影响（尚无渲染实现）
- Q28 定为严格原生分辨率、无开关：逆向更正 [640,1080] 钳位只作用于 Live 画质档，剧情后备缓冲 = 原生分辨率，
  角色层固定 2304×1536 RT 缩放合成。**将决定未来的像素输出**（尚无渲染实现，当前无影响）
- Round 7（Q32–Q39）：IR v1 定稿（`docs/spec/ir.md`）——薄 IR、按帧模拟、按游戏帧率模拟（数值待逆向）、
  Unsupported 参与调度、保留分支树（仅 Action=5）、调试 JSON、无人工覆盖、玩家名默认「「世界」的居民」
- Q20 二进制 fixture = Git LFS
- Q21 CI = 暂不启用，仅手动触发
- Q22 项目语言 = 代码与 commit 英文 / PR·Issue 中英皆可 / 设计文档中文 / 门面双语

### 待定 / Open
- `docs/spec/ir.md`、`docs/spec/param-table.md` 尚未设计
- T1（跨平台帧哈希比对）需自建带 NVIDIA 卡的 runner；当前靠 PR 附本地报告
