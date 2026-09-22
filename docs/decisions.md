# 决策记录

> 本文件是项目的单一事实来源。任何与此冲突的代码或文档以本文件为准。
> 决策变更时**追加修订条目**，不要原地覆盖——历史上下文本身有价值。

状态：v1（Round 1–5 完成）

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
| Q8 | Live2D 语义 | 照搬 **Cubism Native Framework** 语义，用 Rust 重写 | ⚠️ 已知风险：游戏用的是 Cubism SDK for Unity，语义不同。见 `risks.md` |
| Q9 | 分辨率范围 | **全范围**：含 safe area、含 aspect clamp | 前置工具：UI 层次 dumper（否则是人肉考古） |
| Q10 | 内容范围 | **仅 Live2D 2D 对话剧情**，其余占位 | 待全量剧本统计出来后回看是否调整 |
| — | 分支处理 | IR 保留分支树；导出默认走第一项，支持 `--branch` / `--all-branches` | 分支在时间轴编译阶段拍平，内核不感知 |
| Q11 | 文本 | atlas 优先 + 游戏自带 TTF 生成 SDF 作 fallback | **技巧**：hook `TryAddCharacters` 预填 atlas，力争消灭 fallback |
| Q12 | 口型同步 | **挂起**，待逆向确认游戏实现方案 | 见 `reverse/open-questions.md` #11 |
| Q13 | 宿主语言 | **Rust** | 反编译的 C# 需翻译；用 .NET 跑原 C# 当测试 oracle 对冲风险 |
| Q14 | 导出架构 | **两阶段**：Pass1 烘焙参数表 → Pass2 无状态并行渲染 | A（单阶段顺序）作为免费 fallback（`--fused`） |
| Q15 | 输出形态 | 全部实现，**做成参数开关**，分层默认关闭 | draw call dump 须窄条件触发，不可布尔常开 |
| Q16 | 版本更新 | 版本锁定，更新后**手动适配** | 建议仍把常量外置成数据文件（见下方注记） |
| Q17 | 交付形态 | **开源代码**，资产从 Web Assets Source 抓取或本地自备 | 绝不分发资产；需资产源抽象 + 缺资产明确报错 |
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

## 配套规约

Q1–Q18 的执行细则已拆分为以下规约文档，**它们与本文件具有同等约束力**：

| 规约 | 落实的决策 |
|---|---|
| [`conventions/determinism.md`](conventions/determinism.md) | Q6（双端 GPU + 放宽容差） |
| [`spec/tolerance.md`](spec/tolerance.md) | Q2 / Q7（验收线与容差） |
| [`spec/coordinate-systems.md`](spec/coordinate-systems.md) | Q9（全分辨率范围）、Q14（时间基） |
| [`spec/glossary.md`](spec/glossary.md) | Q17（开源协作） |
| [`reverse/workflow.md`](reverse/workflow.md) | Q16（版本锁定 + 手动适配） |
| [`versioning.md`](versioning.md) | Q16 |
| [`testing.md`](testing.md) | Q2 / Q14 |
| [`../CONTRIBUTING.md`](../CONTRIBUTING.md) | Q17 / Q18 |
