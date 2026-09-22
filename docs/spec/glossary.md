# 术语表

> 本项目混用中文、日文原文、英文和游戏内部命名。**同一概念在全项目只能有一个词。**
> 新增术语必须先进本表再进代码。评审时发现表外术语应打回。

## 剧本与演出

| 规范术语 | 游戏内部命名 | 中文 | 定义 |
|---|---|---|---|
| Scenario | `ScenarioData` | 剧本 | 一段完整剧情的数据单元 |
| Snippet | `ScenarioSnippet` | 片段 | 剧本的最小有序事件单元 |
| Action | `SnippetAction` | 动作类型 | Snippet 的种类（Talk / Layout / SpecialEffect / Sound …） |
| ProgressBehavior | `SnippetProgressBehavior` | 推进行为 | `Now`（不等待，并发）/ `WaitUntilFinished`（阻塞） |
| Talk | `TalkData` | 对话 | 一句台词，含说话人、正文、语音 |
| Layout | `LayoutData` | 站位 | 角色的出场/退场/移动/景深/动作/表情 |
| SpecialEffect | `SpecialEffectData` | 特效 | 黑场、Telop、全屏文字、换背景、Sekai 转场等 |
| Telop | — | 字幕板 | 叠加在画面上的场景说明文字 |
| Sekai | セカイ | — | 游戏世界观概念；剧情中有 `SekaiIn` / `SekaiOut` 转场 |
| Flashback | — | 回想 | 带色调与 vignette 的回忆演出 |
| Selectable | — | 选项 | 分支选择点 |

## 角色与资产

| 规范术语 | 游戏内部命名 | 定义 |
|---|---|---|
| Character2dId | `Character2dId` | 剧本引用角色的 ID，需经 master data 映射到模型资源名 |
| Costume | — | 服装差分，体现在模型资源名里 |
| Motion | `MotionName` | **身体**动作 |
| Facial | `FacialName` | **表情**，与 Motion 是独立的两组参数，可叠加 |
| Side | `SideFrom` / `SideTo` | 横向站位枚举，需查表得到实际坐标 |
| Depth | `DepthType` | 景深层级 → scale + 排序 +（待确认）压暗 |

> ⚠️ Motion 与 Facial 在中文里都容易被叫成「动作」，**禁止**。中文文档里分别写「身体动作」和「表情」。

## Live2D

| 规范术语 | 定义 |
|---|---|
| moc3 | Cubism 模型二进制，由 Cubism Core 解析 |
| motion3 | 动作曲线 JSON |
| physics3 | 物理设定 JSON |
| ArtMesh | Cubism 的可绘制网格单元 |
| Drawable | Core 输出的一个可绘制对象（顶点/索引/贴图索引/混合模式/遮罩） |
| Core | Live2D Cubism Core，闭源 C 库，仅负责 moc3 解析与顶点变形 |
| Framework | Cubism 的上层运行时（动作/物理/表情/渲染），本项目用 Rust 重写 |

## 本项目自有概念

| 规范术语 | 定义 |
|---|---|
| **IR** | 中间表示。解析层与编译层之间的契约，见 `spec/ir.md` |
| **Timeline** | 绝对时间轴。IR 经编译后每个事件带确定的帧号区间 |
| **ParamTable** | 参数表。Pass 1 烘焙出的逐帧完整状态快照，见 `spec/param-table.md` |
| **Bake** | Pass 1：顺序空跑时间轴、只更新状态、输出参数表 |
| **Ground truth** | 真值。来自 PlayCover 运行的游戏本体 |
| **Oracle** | 测试参照物。用 .NET 跑反编译出的 C# 产生期望值 |
| **Dumper** | 注入游戏进程提取数据的工具（UI 层次 / 参数 / atlas / 原始帧） |
| **Fixture** | 固化的测试数据（dumper 或 oracle 的产出） |
| **T1** | 跨平台一致性容差（macOS 渲染器 vs Windows 渲染器） |
| **T2** | 对游戏保真度容差（macOS 渲染器 vs ground truth） |
| **Provenance** | 逆向结论的来源记录，见 `reverse/workflow.md` |
| **差异形态** | diff 的分类：边缘带 / 区块 / 孤点 / 全局偏移。比差异幅度更重要 |

## 易混淆词对照

| 不要写 | 要写 | 原因 |
|---|---|---|
| 分辨率 | 参考分辨率 / 目标分辨率 / SSAA 倍率 | 三者必须区分（见 `coordinate-systems.md`） |
| 缩放 | `scaleFactor`（CanvasScaler）/ SSAA 倍率 / 模型 scale | 同上 |
| 时间 | 帧号 / 秒 / 采样 | 权威是帧号 |
| 动作 | 身体动作 / 表情 | Motion vs Facial |
| 一致 | 逐位一致 / 容差内一致 | 本项目已放弃逐位一致（Q6=D） |
