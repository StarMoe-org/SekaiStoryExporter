# Changelog

格式参考 [Keep a Changelog](https://keepachangelog.com/)。

**本项目的特殊约定**：任何会改变输出像素的变更，必须在此标注并注明影响范围。
**Project-specific rule**: any change that alters output pixels must be recorded here
together with its impact.

## 未发布

### 新增 / Added

- **S3**：`--library s3://bucket/prefix` 直接读取 SekaiStoryRipper 发布到 S3（或 MinIO、R2 等兼容服务）的 library，按 episode 只同步所需文件到本地缓存（`--cache-dir` / `SSE_CACHE_DIR`）；`-o s3://bucket/key` 渲染完成后上传视频与报告。凭据只从 `AWS_*` 环境变量读取。见 ADR-0015。无像素影响（与本地 library 逐字节相同）。
  `--library` and `-o` accept `s3://` URLs; only the files an episode needs are fetched.
- **角色着色器（特效 22）**：`SpecialEffectChangeCharacterShader` 的 "hologram" / "monitor" 给角色的 `RawImage` 换上
  `Live2D/Materials/Live2DHologram` 材质（单色化偏青、整体透明度 0.85–0.9 随机闪烁、扫描线亮度），逻辑按 `Live2DHologramController.Update` 逐帧重现；
  "hologram" 同时把 bundle 里的特效 prefab 挂到角色的模型视图下（随角色移动、缩放；角色退场淡出结束时隐藏，再次登场时重新播放）；"none" 移除。
  扫描线贴图 `holo.png` 由 `tools/ui-kit/extract.py` 从客户端导出。参数表 v5。
  **像素影响**：使用全息效果的剧集（如日服活动 217 第 3、4、7 话）中，对应角色显示为全息投影并带粒子特效；此前按普通角色绘制。
  Character shader "hologram" / "monitor" is rendered (material, flicker, attached particle prefab).
- **镜头移动 / 缩放、背景模糊（特效 42 / 43 / 44）**：42 `ScenarioStudioCamera.CameraMove` 对正交的剧情相机做 `DOLocalMove`（参考像素），
  43 对 `scenarioRoot` 做 `DOScale`（背景、角色、特效一起缩放，UI 层不动），44 给背景换 `UIGaussianBlur` 材质（横竖各 7 采样，
  权重 0.036/0.113/0.216/0.269，`_SamplingDistance` 2.8）。角色着色器 "blur" 按 `Live2DBlurController` 与 `Live2DBlur` 材质实现。
  **像素影响**：使用这些特效的剧集中画面平移、缩放、背景或角色变模糊；此前忽略。
  Camera move / zoom, background blur and the character blur shader are rendered.
- **Dolly zoom（日服特效 45）**：解析 `Zoom：z, Blur：b, Dist：d`，背景父节点按 `StringValSub` 缓动 `DOScale`，
  背景换 `UIDollyZoomEffect`（桶形畸变 + 高斯模糊，畸变强度 = Dist × −(Zoom − 1)），`ShouldClearBlur` 逻辑与游戏一致。新增 DOTween 缓动曲线。
  **像素影响**：日服使用 dolly zoom 的剧集背景放大并畸变模糊；此前忽略。
  Dolly zoom (JP effect 45) is rendered.
- **简单选项（特效 23）**：`AnswerChoiceDialog` 按预制体布局绘制（520×96 按钮、FOT-RodinNTLGPro-EB 32 号字），0.125 s 缩放出现 / 消失；
  导出时在对话框打开 1.5 s 后自动选第一个选项（玩家输入，写入报告），选择后等 0.5 s 结束。UI 套件新增 `btn_round_h80_wh`。
  **像素影响**：含选项的剧集显示选项对话框，时长相应增加；此前立即跳过。
  Simple selectable choices are shown; the export picks the first answer after 1.5 s.
- **非出场 Layout 的换装**：`CheckAndChangeCostume` 在按类型分支前执行：服装不同时先用一帧淡出到 0 并隐藏，再换模型。IR v4：`Layout.costume`。
  **像素影响**：剧中换装的角色现在换成新服装（此前一直是旧服装）。
  Costume changes on layouts of every type swap the model, as the game does.
- **BGM 音量渐变（Sound PlayMode 4）**：`AisacVolumeBGM`（类别 AISAC `VOL_BGM_SCE`，线性）按 Duration 渐变，参数表 v5 `bgm_volume`。无像素影响，BGM 音量变化。
  BGM volume tweens are mixed.
- **互动 BGM（Sound PlayMode 5 / 6）**：读取 ACB 的 block sequence（每个 block 的长度、循环、切换时机、各轨波形）和各层 AISAC。
  PlayMode 6 `SetBgmBlockIndex` 在当前 block 为奇数时等待，再切换；循环 block 重复，其余播放 NumLoops + 1 次后前进；
  切换时机 1 时在 block 的 N 等分点切换（CRI 运行时未逆向，近似，写入报告）。PlayMode 5 `BGM_VERTICAL` 调各层音量。
  此前把一个分块 BGM 的全部波形同时混音。无像素影响。
  Interactive (block) BGMs play block by block with their vertical layers.
- **特效动画事件**：`CommandAnimator` 的 `OnPlayParticle` / `OnStopParticle`（播放 / 停止指定路径的粒子），
  `OnPlayEnvironmentSE` / `OnStopEnvironmentSE` / `OnSetEnvironmentSEFadeTime` / `OnSetEnvironmentSEVolume` / `OnPlaySE`（特效自带 ACB 的音效）。
  **像素影响**：靠动画事件启动的粒子现在出现；特效音效（烟花声等）现在播放。
  Effect animation events start / stop particles and play the effects' own sounds.
- **粒子系统补全**：Mesh 渲染（内置 Quad / Cube 与 bundle 内网格）、3D 起始旋转与分轴旋转、Birth 子发射器（遵守子系统 startDelay）、
  限速按总速度计算（阻尼按 1/30 s 步长，近似）、发射器旋转、World 模拟空间、SizeBySpeed / RotationBySpeed、Donut 与 Sprite 发射形状（Sprite 按矩形近似）、
  `ParentRectFitter`，以及拖尾（TrailModule，Particles 模式：按 minVertexDistance 记录轨迹点，宽度 / 颜色随拖尾与寿命变化，使用渲染器第二个材质）。
  **像素影响**：烟花、流星、斩击等特效的形状与运动接近游戏（此前网格粒子是平面方块、烟花不爆炸、流星没有尾巴）。
  Particle systems gain mesh rendering, 3D rotation, sub-emitters, world space, speed modules, more shapes and trails.
- **SpriteMask**：`SpriteRenderer` / `ParticleSystemRenderer` 的 `m_MaskInteraction` 1 / 2 只在遮罩精灵（alpha ≥ `m_MaskAlphaCutoff`）内 / 外绘制，
  自定义范围按排序 (back, front] 生效。依赖 SekaiStoryRipper 导出 `_textures/`（遮罩使用的内置 `Square` 等无 container 路径的贴图）。
  **像素影响**：cut-in 等特效的光效只出现在斜向条带内；饮料杯后的三角形被杯子遮住。
  SpriteMask is applied to effect sprites and particles.
- **更多特效动画属性**：`localEulerAnglesRaw` x / y / z（按 Unity 的 z-x-y 顺序做 3D 旋转，正交相机只看到 x / y 投影，静态的 3D 旋转也一样生效）、
  `RectTransform.m_LocalPosition.z`、`SpriteRenderer.m_Color` 与 `material._Color`、粒子的 `looping`（关掉后播完当前一轮）、
  爆发数量 `m_Bursts[i].countCurve.scalar`、起始颜色 `startColor` min / max。
  **像素影响**：聚光灯等绕 x / y 旋转的精灵按透视缩短；cut-in 角色图随动画淡入淡出；雨在停止时不再爆发新雨滴。
  More animated effect properties drive the nodes and particle systems.

### 修复 / Fixed

- **Additive+AlphaBlend 粒子**：`Sekai/Particles/Additive+AlphaBlend` 按顶点流 Custom1.x 逐粒子选择叠加或 alpha 混合（`ParticleShaderSettings` 设置），
  此前全部按 alpha 混合。**像素影响**：这类特效中的叠加粒子变亮。
  Additive+AlphaBlend particles pick additive or alpha blending per particle.
- **Layout 抖动（类型 4 / 5）**：游戏抖的是模型视图下的空节点 `shake`，画面上不动；但 snippet 要等抖动结束（0.5 / 0.75 / 0.25 s）才完成。
  **像素影响**：无；含角色抖动的剧集节奏变慢相应秒数。
  Layout shakes move nothing on screen but now take their time, as in the game.
- **角色着色器的其他字符串**：游戏只识别 hologram / monitor / blur / none，其他字符串（如空串）直接结束，不再报告为未支持。无像素影响。
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
- **粒子起始颜色 RandomColor**：`MinMaxGradient` 状态 4（`RandomColor`）按每个粒子的随机数在渐变上取色，此前误当作普通渐变在 t = 0 取色，所有粒子都是第一个关键色。
  另外，主模块中以曲线/渐变给出的起始值（寿命、速度、大小、旋转、颜色）改为按发射时刻的系统归一化时间取值（此前恒取 t = 0）。
  **像素影响**：全息特效的三角形由单一青色变为青、黄、品红三色随机（与 prefab 数据一致）；本地日服库中其他剧情特效 prefab 不受影响。
  Particle start colours in RandomColor mode now pick a random point on the gradient.

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
