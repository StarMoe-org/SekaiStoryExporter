# 决策记录

> 本文件是项目的单一事实来源。任何与此冲突的代码或文档以本文件为准。
> 决策变更时**追加修订条目**，不要原地覆盖——历史上下文本身有价值。

状态：v3（Round 1–7 完成）

---

## 决策总表

| # | 议题 | 决策 | 关键后果 |
|---|---|---|---|
| Q1 | FFI 边界 | 可 FFI，**不编译 C++ 源码** | Cubism Core（纯 C ABI）可链接；Cubism Framework 必须用 Rust 重写 |
| Q2 | 验收线 | ① 任意分辨率与游戏像素级对应<br>② macOS / Windows 双端一致 | 需要 diff harness；需要完整 UGUI 布局复刻 |
| Q3 | 平台 | macOS (Apple Silicon) + Windows (Intel CPU / NVIDIA GPU)，**各自优化** | 两条导出路径；两端性能瓶颈不同（统一内存 vs PCIe 回读） |
| Q4 | 产物形态 | 共用无状态内核 `render(frame_n)` | 预览器与导出器只是两个驱动循环 |
| Q5 | Ground truth | PlayCover (macOS) 跑 iOS 版，分辨率可调 | **建议 hook present 直取原始帧**，消除录屏损失 |
| Q6 | 渲染后端 | **双端 GPU（wgpu）+ 放宽容差** | 放弃逐位一致，改为可度量容差；须执行「确定性纪律清单」 |
| Q7 | 容差规范 | T1（跨平台）严 / T2（对游戏）松；**主判据是差异形态而非幅度** | 见 `spec/tolerance.md` |
| Q8 | Live2D 语义 | ~~照搬 **Cubism Native Framework** 语义，用 Rust 重写~~ → **被 Q24 取代** | R1 已成立，见 `risks.md` |
| Q9 | 分辨率范围 | **全范围**：含 safe area、含 aspect clamp | 前置工具：UI 层次 dumper（否则是人肉考古） |
| Q10 | 内容范围 | **仅 Live2D 2D 对话剧情**，其余占位 | 待全量剧本统计出来后回看是否调整 |
| — | 分支处理 | IR 保留分支树；导出默认走第一项，支持 `--branch` / `--all-branches` | 分支在时间轴编译阶段拍平，内核不感知。**Q36 细化**：只有 Action=5 产生分支 |
| Q11 | 文本 | atlas 优先 + 游戏自带 TTF 生成 SDF 作 fallback | **技巧**：hook `TryAddCharacters` 预填 atlas，力争消灭 fallback |
| Q12 | 口型同步 | ~~**挂起**，待逆向确认游戏实现方案~~ → **被 Q27 解挂** | 见 `reverse/open-questions.md` #11 |
| Q13 | 宿主语言 | **Rust** | 反编译的 C# 需翻译；用 .NET 跑原 C# 当测试 oracle 对冲风险 |
| Q14 | 导出架构 | **两阶段**：Pass1 烘焙参数表 → Pass2 无状态并行渲染 | A（单阶段顺序）作为免费 fallback（`--fused`） |
| Q15 | 输出形态 | 全部实现，**做成参数开关**，分层默认关闭 | draw call dump 须窄条件触发，不可布尔常开 |
| Q16 | 版本更新 | 版本锁定，更新后**手动适配** | 建议仍把常量外置成数据文件（见下方注记） |
| Q17 | 交付形态 | **开源代码**，资产~~从 Web Assets Source 抓取或本地自备~~ → 来源由 **Q25** 细化 | 绝不分发资产；缺资产明确报错 |
| Q18 | 时间尺度 | **长期项目，正确性优先于速度** | 先做逆向确认与工具链，再动渲染器 |

---

## 注记

### 关于 Q16 + Q18 的组合

「手动适配」与「常量外置」不冲突。即便适配过程是人工的，把枚举、常量、坐标表、布局参数放进版本化的 `reverse/constants.yaml`（由 xtask 代码生成到 `.rs`），也能让每次适配从「改代码 + 重编译 + 找漏网之鱼」变成「改 YAML + 跑单测」。在长期项目（Q18=C）下这笔投入回本很快。

### 关于 Q6 的确定性纪律清单

> **已提升为正式规约**：见 [`conventions/determinism.md`](conventions/determinism.md)，
> 其中 D 级规则由 `clippy.toml` 编译期强制，R 级规则进入 PR 评审清单。

以下为原始清单，保留作为上下文：

**着色器侧**
1. 关闭 fast-math（Metal 编译器默认开启；需确认 wgpu 是否暴露 `MTLCompileOptions.fastMathEnabled`，未暴露可能要 patch）
2. 禁用超越函数：不出现 `pow/exp/log/sin/cos/rsqrt/normalize/inversesqrt`；归一化用 `v / sqrt(dot(v,v))`
3. 全程 f32，禁 f16
4. FMA 显式化：要么处处写 `fma()`，要么用临时变量阻止合并

**管线侧**

5. 禁用 MSAA，改 2×/4× SSAA + 自写固定权重 box 降采样
6. 禁用各向异性过滤
7. 不用 GPU 自动生成 mipmap，离线预生成显式上传
8. UI 层手动双线性（`textureLoad` 取 4 texel 自己插值）
9. Render target 用 RGBA8Unorm / RGBA16Unorm，不用 fp16 RT

**CPU 侧（最易忽略）**

10. draw call 顺序必须确定：用 `BTreeMap` / `IndexMap`，**不要用 `HashMap`**（Rust 迭代顺序随机化，会导致同机两次运行结果不同）
11. 浮点累加顺序固定；多线程 reduce 按固定次序合并

### 关于 Windows 端缺少独立真值

PlayCover 只在 macOS 存在，Windows 端没有独立 ground truth，其正确性靠传递链保证：

```
PJSK(PlayCover/macOS) --T2--> 渲染器(macOS) --T1--> 渲染器(Windows)
```

**缓解**：Windows 上跑安卓模拟器版 PJSK 作二级校验，不进日常 CI，只在大改动后抽查，防止「两端一起错」的盲区。

### 关于 Q9=D 的验证缺口

PlayCover 窗口无刘海，`Screen.safeArea` 大概率等于全屏，**safe area 的适配行为在主 ground truth 环境里验证不了**。
建议方案：hook 伪造 `Screen.safeArea` 返回值，在 PlayCover 内触发刘海分支。


---

## 后续决策（2026-09-22，工程设施层）

| # | 议题 | 决策 | 落地位置 |
|---|---|---|---|
| Q19 | 开源许可证 | **AGPL-3.0-or-later** + Live2D Cubism Core 链接例外 | `LICENSE`、`LICENSE-EXCEPTION`、[ADR-0002](adr/0002-license.md) |
| Q20 | 二进制 fixture 存储 | **Git LFS** | `.gitattributes`、[`testing.md`](testing.md) |
| Q21 | CI | **暂不启用**，配置保留为手动触发；测试全部本地执行 | `.github/workflows/ci.yml`、[`testing.md`](testing.md) |
| Q22 | 项目语言 | **代码与 commit 用 English；PR/Issue 中英皆可；设计文档中文；门面双语** | [`conventions/language.md`](conventions/language.md) |

### Q19 注记：AGPL 与闭源库的兼容性

AGPL 要求分发「组合作品」时提供全部对应源码，而 Live2D Cubism Core 是闭源静态库。
**只分发源码没有问题**（用户自行链接不构成分发），但分发已链接的二进制会与 AGPL 冲突。
因此增加了 `LICENSE-EXCEPTION`（AGPL 第 7 条允许的附加许可）。
若将来接受外部贡献，贡献者须同意其代码同样适用该例外——已写入 `CONTRIBUTING.md`。

### Q23 · 项目改名（2026-09-23）

| # | 议题 | 决策 | 落地位置 |
|---|---|---|---|
| Q23 | 项目名称 | 由 `pjskChatGenerator` 改为 **SekaiStoryExporter**，缩写 **sse** | 仓库名、crate 前缀 `sse-*`、Rust 路径 `sse_*`、CLI 可执行名 `sse`；见 [`spec/glossary.md`](spec/glossary.md) |

注记：`PJSK` 保留为游戏本身的称呼，不再出现在任何标识符中。外部站点名（如 `pjsk.moe`）不属于本项目命名，原样保留。

### Round 6 · 接入 SekaiStoryRipper 与逆向结论复审（2026-09-24）

背景：静态逆向（RE-01…RE-13）推翻了 Q8 的前提，Q12 的挂起条件已满足；伴生项目
[SekaiStoryRipper](https://github.com/StarMoe-org/SekaiStoryRipper)（MIT OR Apache-2.0）发布 v0.1.0，
产出无损的 `sse-motion` v1 与 `ripper-episode` v1。本轮据此复审。

| # | 议题 | 决策 | 关键后果 |
|---|---|---|---|
| Q24 | Live2D 语义（取代 Q8） | **Unity 语义**：曲线来自 `sse-motion`，按 StreamedClip / DenseClip / ConstantClip 求值；层间用 `PlayableBlender` **线性**交叉淡化（身体 0.5 / 表情 0.25 / 同类别 0.125 s）；模型变形仍走 Cubism Core 4.1 FFI | `MotionEvaluator` 只剩 Unity 一种实现；Native Framework 的 motion/fade 逻辑不移植。physics、遮罩按 `live2d.md` 复刻 |
| Q25 | 资产来源（细化 Q17） | sse **只消费 Ripper 的输出**（`library/` + `episodes/`），自身不含任何 CDN / 解密代码 | ABCrypt 密钥永远不进入 sse；`tools/fetch` 取消；缺资产时报错并提示 `ripper rip <selector>` |
| Q26 | 格式依赖方式 | `ripper-format` 作 **git 依赖，钉 tag**（首个：`v0.1.0`） | 升级 tag = 一次显式变更，写 CHANGELOG；`sse-assets` 对未知 `format`/`version` 一律拒绝。仓库目前私有，构建需 SSH 访问权限 |
| Q27 | 口型同步（解挂 Q12） | **现在实现**：有语音 → RMS 经 `a·(a+1)^powK`（powK 1.75）+ 阈值 + 三段平滑；无语音 → `LipLevels[38]` 每半字推进 | 输入为 Ripper 解出的 WAV；**HCA 解码未独立验证**，登记为 open-question #38，不阻塞实现 |
| Q28 | 「任意分辨率」的定义（复审 Q2 / Q9） | ~~**暂缓**，等 PlayCover 实测~~ → **2026-09-24 定：严格原生，无开关**（见下方修订） | — |
| Q29 | 动作 / 表情包解析归属 | **信任 episode 索引的 `motions` 表**；sse 只做一致性校验，不重新实现解析规则 | 规则只在 Ripper 一处实现（见其 `live2d-bundle-resolution.md`）；索引缺项 = 游戏保持上一动作，sse 照此行为 |
| Q30 | 逆向结论的归属 | **两边各管各的，互相链接**：游戏运行时行为 → sse `docs/reverse/`；资产打包 / bundle 命名规则 → Ripper `docs/reverse/` | sse `reverse/versions/cn-6.4.0/README.md` 维护指向 Ripper 文档的链接表 |
| Q31 | 首个里程碑 | **M1 = CPU 确定性链路**（索引 → 剧本 → IR → 时间轴 → Pass 1 参数表 → `sse inspect` / `sse params` + golden test）；**M2 = 静态首帧**（Core FFI + wgpu，单角色 PNG） | 先写 `spec/ir.md`、`spec/param-table.md`；M1 可用 RT-02 参数 dump 校验，无需 GPU |

#### Q24 注记：为什么不保留 Native 语义作为可选实现

资产里没有 motion3.json；第三方转出的 motion3 已知有损（AssetStudio 把零切线三次段写成 Linear，
最大偏差约 9.5 个参数单位）。Native 语义没有忠实的输入可吃，保留它只会产生一条永远过不了 T2 的路径。

#### Q25 / Q26 注记：许可证方向

`ripper-format` 是 MIT OR Apache-2.0，被 AGPL-3.0-or-later 的 sse 依赖没有问题；反方向（Ripper 引用 sse 代码）不允许。
sse 只依赖 `ripper-format`（仅 serde），**不**依赖 `ripper-cdn` / `ripper-unity` 等 crate，这是 Q25 的结构保证。

#### Q29 注记：场景剧本名

Ripper 实测（其 `story-asset-rules.md` §0）：拼 bundle 名必须用 masterdata 的 `scenarioId`，
而非剧本 JSON 内的 `ScenarioId`（CN 有 54 话不一致，其中 24 话会静默播错语音）。
sse 从索引拿路径，天然规避；**sse 内部任何地方都不得用剧本内 `ScenarioId` 反查资产。**

#### Q28 修订（2026-09-24）· 严格原生，无开关

静态逆向（[`reverse/notes/2026-09-24-render-resolution.md`](reverse/notes/2026-09-24-render-resolution.md)）更正了 Q28 的前提：
[640,1080] 钳位**只作用于 Live 画质档**；剧情从不调用 `Screen.SetResolution`，后备缓冲 = 原生分辨率。

| 决策 | 后果 |
|---|---|
| 输出分辨率 = 游戏的「原生屏幕」；UI / 背景 / 文字按目标分辨率原生光栅化；**角色固定渲染进 2304×1536 RT 后按 `ContentSize.y / 1024` 缩放合成**，高分辨率下的角色软化一并复刻 | `sse-render` 不做高度钳位、不做内部缩放；**不提供**角色超采样或任何非还原分辨率开关。RT 放大时的采样方式由 RT-03 帧级核对 |
| 取消 Round 6 中「分辨率入口按可切换策略设计」的临时要求 | 只有一条分辨率路径 |

PlayCover 的原生分辨率 = 窗口点数 × `customScaler`（默认 1920×1080 pt × 2 = 3840×2160），ground truth 可覆盖任意分辨率。
运行时残留项（`Screen.dpi` 实值、IFix 热补丁）不影响剧情结论，归 RT-02 验证。

---

### Round 7 · IR 设计（2026-09-24）

逐题结论落地在 [`spec/ir.md`](spec/ir.md)。

| # | 议题 | 决策 | 关键后果 |
|---|---|---|---|
| Q32 | IR 抽象层级（IR-1） | **薄 IR**：规范化的带类型指令序列，保留原下标 / `ProgressBehavior` / `Delay` / 完成规则；**不含绝对时间** | 时序完全由 `sse-timeline` 模拟；语音时长不进 IR |
| Q33 | 时间轴求解（IR-2） | **按帧模拟**：固定步长复刻 Unity 协程（`yield null`、`WaitForSeconds`、`elapsed += deltaTime` 用 f32） | 秒 → 帧不再「round 一次定死」；量化由逐帧累加自然产生。`spec/coordinate-systems.md` 时间基一节已改 |
| Q34 | 模拟帧率与导出帧率（IR-3） | **固定按游戏帧率模拟**，导出帧**按游戏自身的帧同步策略**由模拟帧得到 | 模拟帧率数值与帧同步规则均**待逆向**（open-question #40）；确认前不写默认值 |
| Q35 | 不支持的内容（IR-4） | `Unsupported{reason, finish, raw}`，**参与调度**，完成语义照游戏，画面占位 | 如 EffectType ≥ 45 → 立即完成；Movie 等无法静态确定的记 `ApproximateTiming` |
| Q36 | 分支（IR-5） | **保留分支树**；**只有 Action = 5 产生 `Branch`**，其语义待逆向，暂按 Unsupported 调度并报警；SimpleSelectable (23) 是普通特效 | `--branch` / `--all-branches` 保留；open-question #41（SimpleSelectable 在自动模式下的行为）、#42（Action = 5 语义） |
| Q37 | IR 持久化（IR-6） | Rust 类型 + 带 `ir_version` 的调试 JSON（`sse inspect`），**不承诺稳定外部格式** | 改 IR 即 bump 版本、更新 golden，不做旧版兼容 |
| Q38 | 人工覆盖（IR-7） | **v1 不做**，1:1 复刻（含游戏自身数据错误） | 如 CostumeType `"3"` → 与游戏同样加载失败 |
| Q39 | `{{playerName}}`（IR-8） | 配置 `player_name`，**默认 `「世界」的居民`**，`--player-name` 覆盖 | 替换在 IR 构建时完成（对应 `CreateFinalSerifBody`） |

---

## 配套规约

Q1–Q39 的执行细则已拆分为以下规约文档，**它们与本文件具有同等约束力**：

| 规约 | 落实的决策 |
|---|---|
| [`conventions/determinism.md`](conventions/determinism.md) | Q6（双端 GPU + 放宽容差） |
| [`spec/tolerance.md`](spec/tolerance.md) | Q2 / Q7（验收线与容差） |
| [`spec/coordinate-systems.md`](spec/coordinate-systems.md) | Q9（全分辨率范围）、Q14（时间基） |
| [`spec/glossary.md`](spec/glossary.md) | Q17（开源协作） |
| [`reverse/workflow.md`](reverse/workflow.md) | Q16（版本锁定 + 手动适配） |
| [`versioning.md`](versioning.md) | Q16 |
| [`testing.md`](testing.md) | Q2 / Q14 |
| [`conventions/language.md`](conventions/language.md) | Q22 |
| [`../CONTRIBUTING.md`](../CONTRIBUTING.md) | Q17 / Q18 / Q19 / Q22 |
