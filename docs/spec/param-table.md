# ParamTable 格式 v3

> **状态：v3（2026-09-24）。** v2 → v3：新增 `fx`（`FxState { age_frames, seed }`，Sekai 转场粒子实例；
> 渲染端按 `age_frames` 从头重放模拟，保持无状态）。
> v1 → v2：`full_screen_text` 改为 `FullScreenTextState`
> （`progress` 逐槽进度 + `alpha` 淡出），新增 `cinemascope`（FST 黑边/底板的缓动比例）、`menu_alpha`、
> `TalkState.auto_time`（AUTO 三角闪烁相位）；`movie` 改为 `MovieState`（视频文件 + 已播放秒数）。 类型定义在 `crates/sse-params`，本文件说明语义。
> 参数表是 Pass 1（`sse-bake`）与 Pass 2（`sse-render` / `sse-export`）的接缝（Q14）。

## 结构

| 层 | 内容 |
|---|---|
| `ParamTable` | `version`、`fps`（= 模拟帧率 60）、`models`（本话用到的模型 bundle）、`frames`、`audio`、`notes` |
| `FrameState`（每帧一份，完整快照） | 背景（当前 / 上一张 + 交叉淡入权重）、角色列表（**按绘制顺序**）、`ColorFader` 颜色、模糊量、相机色调、对话框、Telop / 地点 / 全屏文字（逐字进度）、FST 黑边、菜单按钮、影片占位、转场粒子实例 |
| `CharacterState` | 模型下标、不透明度、UI 坐标（1920×1080 参考空间，底边中点）、模式缩放、环境色、**全部 Cubism 参数的最终值**（按模型参数顺序） |
| `AudioCue` | 波形文件、起止帧、循环、音量、淡入淡出帧数、类别 |

## 约定

- **渲染只读这张表。** `sse-render` 不依赖 `sse-timeline` / `sse-scenario`，保证 Pass 2 无状态（架构铁律）。
- Cubism 参数是动作 + 表情 + 眨眼 + 口型 + 呼吸 + 物理之后的值，渲染端只需 `csmUpdateModel`。
- 文字只存「可见 UTF-16 单元数」，与游戏 `Substring(0, n)` 一致；排版由渲染端完成。
- `notes` 汇总所有近似与未支持项，导出时写入 `*.report.txt`。

## 尚未实现

- delta + zstd 编码与磁盘持久化（当前只在内存中）
- 与游戏 hook dump（RT-02）的比对协议：字段对应、容差、缺失字段处理
- 随机访问索引（并行 Pass 2 / 预览器 seek）
