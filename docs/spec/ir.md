# IR 规约 v1

> **状态：v1 定稿（2026-09-24）。** 依据 [ADR-0008](../adr/0008-ir.md)。

IR（Intermediate Representation，中间表示）是 `sse-scenario`（解析）与 `sse-timeline`（调度模拟）之间的接缝：

```
ripper-episode 索引 + ScenarioSceneData JSON
        │  sse-scenario：解析、规范化、资产引用落地
        ▼
       IR   ← 本文件（类型定义在 sse-ir）
        │  sse-timeline：按帧复刻 Unity 协程调度（ADR-0009）
        ▼
   绝对时间轴 → sse-bake（Pass 1 参数表）→ sse-render（Pass 2）
```

---

## 1. 原则

| # | 原则 | 来源 |
|---|---|---|
| P1 | **IR 不含任何绝对时间。** 只记录「做什么、何时可以开始、怎样算完成」；起止帧由 `sse-timeline` 模拟得到 | ADR-0008 |
| P2 | **一条 snippet 对应一条指令**，保留原下标、`ProgressBehavior`、`Delay`，顺序不变 | `scenario-player.md` §1 |
| P3 | **按游戏实际消费的字段建模**。恒为 0 / 被忽略的字段不进 IR（`Speed`、`FontSize`、`TalkTention`、Move 的 `SideFrom`…），但在 §7 列明去向 | `data-model.md`、2650 剧本统计 |
| P4 | **`StringVal` 按 `EffectType` 解析成带类型的载荷**，不以通用 string 流入下游 | `special-effects.md`「对 IR 的建议」 |
| P5 | **不支持的内容保留且参与调度**：画面占位，完成语义照游戏 | ADR-0008 |
| P6 | **1:1 复刻，包括游戏自身的数据错误**；v1 无人工覆盖 | ADR-0008 |
| P7 | 浮点字段以 **f32** 读入（与 Unity 反序列化一致），不经 f64 中转 | determinism |

---

## 2. 顶层结构

```rust
pub struct Episode {
    pub ir_version: u32,               // = 2
    pub source: EpisodeSource,         // selector、scenarioId（masterdata 侧）、assetVersion
    pub cast: BTreeMap<CharacterId, CastEntry>,
    pub initial: InitialState,
    pub root: Block,                   // 指令树（§5）
    pub diagnostics: Vec<Diagnostic>,  // 解析期发现的问题，不中断
}

pub struct InitialState {
    pub background: Option<BackgroundRef>,        // FirstBackground
    pub bgm: Option<AudioRef>,                    // FirstBgm
    pub layout_mode: LayoutMode,                  // FirstCharacterLayoutMode
    pub layout: Vec<InitialPlacement>,            // FirstLayout
    pub music_video: Option<String>,              // EpisodeMusicVideoId → 仅用于 Unsupported 占位
}
```

- `EpisodeSource.scenario_id` **取自 ripper 索引（masterdata `scenarioId`）**，不取剧本 JSON 的 `ScenarioId`。
  JSON 内的值只作为 `diagnostics` 里的对照信息保留。
- `FirstAisacValue`：2650 剧本全部为空，v1 不建模；非空时记 `Diagnostic::IgnoredField`。
- 容器一律 `BTreeMap` / `Vec`（determinism D 级规则）。

### 2.1 角色与资产解析

```rust
pub struct CastEntry {
    pub asset_name: Option<String>,
    pub motion_bundle: Option<String>,
    pub costumes: BTreeMap<CostumeType, CostumeEntry>,   // 来自 ripper 索引
}
pub struct CostumeEntry {
    pub model: Option<ModelRef>,                         // None = 游戏里加载失败（P6）
    pub motions: BTreeMap<MotionName, MotionRef>,        // 已按「模型包 → 动作包」解析（ADR-0007）
}
```

指令里的动作 / 表情只写 **`(CharacterId, MotionName)`**，**不在 IR 里落到文件**：
同一名字在不同服装下可能解析到不同 clip，而当前服装是运行时状态（`Apper` 时 `CheckAndChangeCostume`）。
`sse-timeline` 用「当前服装」查 `cast[id].costumes[ct].motions[name]`；查不到 = 该层保持上一动作（游戏行为）。

资产引用类型（`ModelRef`、`MotionRef`、`BackgroundRef`、`AudioRef`、`MovieRef`）都是 ripper `library/` 相对路径的 newtype，
由 `sse-assets` 在解析期校验存在性。**缺失不在解析期报错退出**，而是：

- 游戏里同样会失败的（索引 `warnings` 已列出，如 `ModelBundleMissing`）→ `Diagnostic` + 照游戏行为；
- 索引说存在、磁盘上却没有的 → **硬错误**，列出清单并提示 `ripper rip <selector>`（ADR-0006）。

---

## 3. 指令

```rust
pub struct Instr {
    pub index: u32,                    // Snippets[] 下标（= Snippet.Index，二者不一致时记 Diagnostic）
    pub progress: Progress,            // Now | WaitFinished
    pub delay: f32,                    // 秒，前置；<= 0 不消耗帧
    pub delay_tap_skippable: bool,     // 仅 Sound = true（对自动导出无影响，保留语义）
    pub kind: InstrKind,
}
```

`progress` 的语义方向：`WaitFinished` = **本条开始前**等在跑集合清空（`scenario-player.md` §2）。

### 3.1 `InstrKind`

| Action | IR 变体 | 载荷要点 |
|---|---|---|
| 0 None | `Wait` | 无 |
| 1 Talk | `Talk(Talk)` | 见 §3.2 |
| 2 CharacerLayout | `Layout(LayoutOp)` | `Move{to, offset_x, duration}` / `Appear{from, offset_x, costume?, motion?, facial?, depth}` / `Hide{delay}`（v2：退场淡出前的等待，原地 0.15 s，滑出 = 移动时长 − 0.1 s）/ `Shake{axis, …}` / `Depth(DepthType)` |
| 4 CharacterMotion | `ChangeMotion{character, motion?, facial?}` | `LayoutData.Type = 0`；与 2 共用数组，**按 Type 分流**（`data-model.md`） |
| 6 SpecialEffect | `Effect(Effect)` | 见 §3.3 |
| 7 Sound | `Sound(SoundOp)` | 按 `PlayMode` 分成 `Bgm{…}` / `SeOneShot` / `SeLoop{key,…}` / `Stop{…}` / `BgmVolume` / `BgmAisacVolume` / `BgmBlock` |
| 8 CharacterLayoutMode | `SetLayoutMode(LayoutMode)` | `DefaultMode(0)` / `ThreeMode(3)` |
| 3 InputName | `Unsupported{…}` | 自动导出无改名弹窗 |
| 5 Selectable | `Branch(Branch)` | 见 §5；v1 语义未知，按 Unsupported 调度 |

- `Move.duration` 已由 `MoveSpeedType` 查表落成秒（`layout.yaml`）；`Move` **只用 `SideTo`**。
- `Layout` 指令在调度上额外受 **`busyLayoutCharacter` 同角色串行闸门**约束（`scenario-player.md` §8）；
  这是 `sse-timeline` 的职责，IR 只保证 `character` 字段存在。

### 3.2 `Talk`

```rust
pub struct Talk {
    pub speakers: Vec<CharacterId>,          // TalkCharacters
    pub display_name: String,                // WindowDisplayName
    pub body: TextBody,                      // 见 §6：已替换 {{playerName}}
    pub lip_sync: LipSyncMode,               // Text | Voice | Close
    pub motion_change: MotionChangeFactor,   // Text | PlayTime
    pub motions: Vec<TalkMotion>,            // {character, motion?, facial?, timing_sync_value}
    pub voices: Vec<TalkVoice>,              // {character, voice: AudioRef, volume}
    pub close_window_on_finish: bool,        // WhenFinishCloseWindow
    pub target_value_scale: f32,             // 口型幅度（ADR-0007）
    pub attached_effect: Option<Box<Effect>>,   // RequirePlayEffect（实装恒 false，仍建模）
    pub attached_sound: Option<Box<SoundOp>>,   // RequirePlaySound（同上）
}
```

语音时长**不写进 IR**：`sse-timeline` 通过 `sse-assets` 读取 WAV 得到（P1）。

### 3.3 `Effect`

按 `EffectType` 解析 `StringVal` / `StringValSub` / `IntVal`；`Duration` 对所有类型保留（`special-effects.md`）。

| 类别 | EffectType | IR 载荷 |
|---|---|---|
| 色幕 | 1–4 | `Fade{color: Black\|White, dir: In\|Out, duration}` |
| 抖动 | 5, 6, 25, 26 | `ShakeScreen{duration}` / `ShakeWindow{duration}` / `StopShake…`；`duration = 3600` 保留原值（表示持续） |
| 背景 | 7 | `ChangeBackground{background: BackgroundRef, duration}` |
| 文字 | 8, 18, 24 | `Telop{text}` / `PlaceInfo{text}` / `FullScreenText{text, voice?}` |
| 相机后处理 | 9, 10, 27, 28, 38, 39, 44 | `CameraColor{effect: Flashback\|Sepia}` / `CameraColorOff` / `Blur{dir, duration}` / `BackgroundBlur(bool)` |
| 相机变换 | 42, 43 | `CameraMove{x, y, duration}` / `CameraZoom{scale, duration}`；**解析失败照游戏**（42 段数≠2 → `LogError` 后的行为；43 `TryParse` 失败值） |
| 环境色 | 12–14 | `Ambient(Afternoon\|Evening\|Night)` |
| 场景特效 | 15, 16 | `PlayScenarioEffect{name, bundle}` / `StopScenarioEffect{name}` |
| 角色 shader | 22 | `CharacterShader{character, shader: Hologram\|Monitor\|Blur\|None, bundle, duration}` |
| 转场 | 20, 21, 40, 41, 29–36 | `SekaiTransition{variant, dir}` / `SideFade{fade_type, duration}` |
| 选项展示 | 23 | `SimpleSelectable{options: Vec<String>}`（按 `/` 切分）；**不是分支**（ADR-0008） |
| 空实现 | 0, 11, 17 | `Noop`（游戏里只 `FinishSnippet`） |
| 不支持 | 19 Movie, 37 MusicVideo, ≥45 | `Unsupported{…}`（§4） |

`bool` / `f32` 字符串的解析函数**必须复刻 .NET `Boolean.TryParse` / `Single.TryParse` 的行为**（含失败时取 `false` / `0`），
不得用 Rust 默认解析器替代——实装里有 132 条 `StopShakeWindow` 的 `StringVal` 为空串，依赖的正是失败语义。

---

## 4. 不支持的内容（ADR-0008）

```rust
pub struct Unsupported {
    pub reason: UnsupportedReason,   // InputName | Movie | MusicVideo | UnknownEffectType(i32) | UnknownAction(i32) | …
    pub finish: FinishRule,          // 照游戏的完成语义
    pub raw: RawSnippet,             // 原始 snippet + 其引用的数据体，逐字段保留
}
```

- **时序必须对**：`finish` 取游戏里该指令的真实完成规则。例如 `EffectType ≥ 45` 在 6.4.0 落进 default，只 `FinishSnippet` → `FinishRule::Immediate`。
- 画面：`sse-render` 画占位（由调试开关控制是否叠加说明文字），**默认不叠加**。
- 完成规则无法静态确定的（如 Movie 取决于影片时长）：`FinishRule` 取可得的最佳近似并记 `Diagnostic::ApproximateTiming`，导出报告里列出。

---

## 5. 分支（ADR-0008）

```rust
pub struct Block { pub instrs: Vec<Node> }
pub enum Node { Instr(Instr), Branch(Branch) }
pub struct Branch { pub index: u32, pub arms: Vec<Block>, pub raw: RawSnippet }
```

- IR 保留树结构；**只有 `Action = 5 (Selectable)` 产生 `Branch`**。
- 2650 剧本中 Action = 5 出现 0 次，分支语义（如何分臂、在哪汇合）**尚未实现**：
  v1 解析器遇到 Action = 5 时产出 **单臂 `Branch`**（原序列不变），调度按 `Unsupported` 处理并报警。
- 拍平在 `sse-timeline` 入口完成（内核不感知）：默认走第一臂；`--branch <path>` 选臂；`--all-branches` 逐一导出。
- `SimpleSelectable (23)` 是普通特效，不产生分支。

---

## 6. 文本

- `{{playerName}}`：用配置 `player_name` 替换，**默认值 `「世界」的居民`**，可用 `--player-name` 覆盖。
  替换在 IR 构建时完成（对应游戏的 `CreateFinalSerifBody`），下游只见最终文本。
- 富文本标签**原样保留**（游戏开启富文本），解析交给 `sse-text`。
- 换行、全角标点、不可见字符不做任何规范化。

---

## 7. 不进 IR 的字段

| 字段 | 原因 | 依据 |
|---|---|---|
| `TalkData.Speed`、`FontSize` | 本版本不参与打字机计算；实装恒 0 | 游戏行为 |
| `TalkData.TalkTention` | 枚举仅 `Normal` | 游戏枚举 |
| `LayoutData.SideFrom`（Move 时） | Move 只用 `SideTo`；实装恒 0 | 游戏行为 |
| `FirstAisacValue` | 2650 剧本全空 | 语料统计 |
| `NeedBundleNames`、`IncludeSoundDataBundleNames` | 资产已由 ripper 索引解析 | ADR-0007 |
| Unity 残留 `m_GameObject` / `m_Script` / `m_Enabled` / `m_Name` | 非剧本数据 | — |

上表字段**出现非预期值时**记 `Diagnostic::IgnoredField`，以便新版本数据变化时被发现。

---

## 8. 诊断

```rust
pub enum Diagnostic {
    IgnoredField { index: u32, field: &'static str, value: String },
    IndexMismatch { index: u32, snippet_index: i32 },
    ScenarioIdMismatch { json: String, masterdata: String },
    RipperWarning(ripper_format::episode::Warning),   // 原样透传
    ApproximateTiming { index: u32, reason: String },
    UnknownEnumValue { index: u32, field: &'static str, value: i32 },
}
```

诊断**从不改变 IR 的语义**，只用于报告。`sse inspect --diagnostics` 输出全部诊断。

---

## 9. 持久化与版本（ADR-0008）

- IR 是 `sse-ir` 中的 Rust 类型，**不是稳定的外部格式**。
- `sse inspect <episode>` 输出带 `ir_version` 的 JSON，用于 golden test 与排查；字段顺序由类型定义决定，输出确定。
- 改动 IR 类型（增删字段、改语义）必须 bump `ir_version` 并更新 golden，但**不做旧版本兼容**。

---

## 10. 时间语义（与 `coordinate-systems.md` 的关系）

IR 中唯一的时间量是**秒（f32）**：`delay`、各 `duration`。它们原样来自剧本或常量表，**不在 IR 里量化**。

量化发生在 `sse-timeline`：以**模拟帧率**逐帧推进，按 Unity 语义累加 `elapsed += deltaTime`（f32），
比较 `elapsed < duration` 决定是否再等一帧。模拟帧率为 60，导出 60 fps 时一个模拟帧对应一个视频帧（ADR-0009）。
