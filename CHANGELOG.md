# Changelog

格式参考 [Keep a Changelog](https://keepachangelog.com/)。

**本项目的特殊约定**：任何会改变输出像素的变更，必须在此标注并注明 fidelity 影响范围。
**Project-specific rule**: any change that alters output pixels must be recorded here
together with its fidelity impact.


## 未发布

- 剧情特效 prefab（特效 15 / 16，#51）：`PlayScenarioEffect` 在 `Duration` 之后把 SSR 库里的 prefab 实例化到
  EffectLayer，`StopScenarioEffect` 走 `CommandAnimator.Stop`（停发射、播 "Stop" 状态、全部粒子消亡后销毁）。
  新增 `sse-render` 的 `effect.rs`（层级 / RectTransform 布局 / Image / SpriteRenderer / Canvas 排序 / Animator）
  与 `particle.rs`（通用 Unity 粒子）。依据见 `docs/reverse/versions/jp-6.8.1/effects.md`。
  **像素影响**：用到这些特效的话（日服 event 185/217 等）出现压暗、白色梦境、闭眼 / 睁眼、速度线、聚光、
  下落光、眩晕剧场等画面；逐粒子位置与游戏不同（随机数不可复现）。CN 第一章不受影响（逐像素相同）。

- 日服：`sse` 按 library 的 `ripper.lock.json` `region` 选字体（jp → `FOT-RodinNTLGPro-DB/EB.otf`，
  cn → 思源黑体 SC），`--game cn|jp` 可覆盖。依据见 `docs/reverse/versions/jp-6.8.1/text.md`：
  日服 TalkWindow 排版、FaceInfo 与 CN 逐项相同，只有源字体不同。

## [Unreleased]

### 修复 / Fixed（2026-09-24，对标原生录制的画面校准）
- 角色纵向位置按二进制还原：RT 底边贴屏幕底边，模型站位含 `fixedStagePosition × orthoSize`（−1.5）；不再截头
- 对话框按 `TalkWindow` prefab 重建（底板渐变、名字横条、AUTO 标签与闪烁三角、右上菜单按钮），sprite 由用户从客户端导出；
  剧情 `ColorFader` 改到对话框与菜单之上
- 全屏文字：黑边与 50% 底板、逐字渐显、56 号左对齐、Underlay 阴影
- 相机模糊按 `RenderBlur` 实现（点采样 1/2 降采样、3 轮、半分辨率纹素偏移）；屏幕上的模糊大小随渲染分辨率变化
- 字号按 em 换算（此前 `ab_glyph` 行高口径导致字小约 30%）；正文自动字号从 44 起算
- Telop 条与地点栏按 prefab 与动画 clip 重建（不再依赖第三方叠层）：Telop 展开/淡出曲线取自 `ac_scenario_telop_v2_*`，
  地点栏 0.2 s OutQuart 滑入，下一句台词结束时滑出
- 动作混合的"同类别"判定改为二进制规则（类别 = `^[a-z]-\w+-\w+` 前缀，普通动作即完整名字；过渡动作 `A_to_B` 也按 0.125 s），
  此前按名字第二段近似，会把同一角色的不同动作误判为同类别
- 对话框开合按二进制：窗口关闭时先 0.2 s 线性淡入再开始打字；只有 `WhenFinishCloseWindow` 的台词结束时 0.2 s 淡出；
  去掉了"淡出/Telop/全屏文字时隐藏对话框"这些无依据的规则（影片期间仍按 `SetHideUI` 0.15 s 隐藏）
- 打字机按 `ShowWords(startFirst: true)`：第一帧就显示 1 个字，打字共 `字数 × 每字帧数`（此前多一轮、晚一个字）；空台词直接结束
- 台词内嵌动作按 `SnippetActionTalk` / `TalkMotionChangeSyncVoiceTime` / `OnTalkWindowOnLetter`：开头 `TimingSyncValue == 0`
  的动作同一帧全部播放（此前每帧一条错开），其余按 PlayTime 协程或 Text 逐字回调；bake 直接用时间线给出的动作时刻
- 地点栏隐藏位置按根画布与 UI 相机数据推出（`DefaultPosX` 混用世界坐标）：16:9 为 −578.22，此前近似 −580
- 文字排版支持 TMP `characterSpacing`（每字 `spacing × fontSize × 0.01`，行宽不含末字字距）；
  AUTO 标签按 `ActiveAutoObject`（自动模式只留一个三角）与 Rodin EB 字形算出的首选宽度 85.26 做横向布局，与录像 ±1 px
- **Sekai 转场粒子**（`fx_transition_scenario`）：按 prefab 的 ParticleSystem 模块做 CPU 模拟（burst、cone 形状、
  Hermite 速度曲线、旋转、颜色随生命周期、图集图块、噪声、限速），两种材质的混合按 shader；画在对话框与白幕之下。
  噪声为近似；发射点按二进制居中，与录像（右下角）不一致，见开放问题 #50。参数表 v3（`FxState`）
- 口型平滑系数按 `Live2DVoice.UpdateParam`：以当前值（而非目标值）是否 < 0.1 选择 (0.6, 0.4)
- 片尾影片按 ffmpeg 解码 `.m2v`，拉伸进居中的 2338×1080 `movieResolution` 矩形（此前是占位文字）
- 参数表 v2：`FullScreenTextState`、`cinemascope`、`menu_alpha`、`TalkState.auto_time`、`MovieState`
- **像素影响**：全片 UI、文字大小、角色位置、模糊强度均改变
- Visuals calibrated against the native capture (character placement, prefab-accurate talk window, FST, blur, font size).

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
