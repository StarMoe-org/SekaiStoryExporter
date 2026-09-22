# 确定性规约

> 来源：决策 Q6（双端 GPU + 放宽容差）。
> 选了 GPU 不等于放弃确定性——本规约的目标是把双端差异压到「少量边缘像素差 1 LSB」，
> 使 `spec/tolerance.md` 的 T1 阈值可达。

## 适用范围

**受约束**（渲染路径）：`pjsk-render`、`pjsk-bake`、`pjsk-live2d`、`pjsk-ugui`、`pjsk-text`、`pjsk-core`

**不受约束**：`pjsk-cli` 的日志与进度显示、`pjsk-assets` 的网络抓取、`tools/*`

判据：**凡是其输出会影响到某一帧像素的代码，一律受约束。**

## 两级规则

- **D 级**：自动检查（clippy / CI），违反即编译失败
- **R 级**：人工检查，列入 PR 评审清单

---

## D 级（自动强制）

| ID | 规则 | 理由 | 检查方式 |
|---|---|---|---|
| D-1 | 禁用 `HashMap` / `HashSet` | Rust 的迭代顺序随机化 → **draw call 顺序不确定 → 同机两次运行结果不同**。这比跨平台差异难查十倍 | `clippy.toml` disallowed-types |
| D-2 | 禁用 `Instant` / `SystemTime` | 渲染路径不得有实时时钟。时间只能来自帧号（见 `spec/coordinate-systems.md` 时间基一节） | 同上 |
| D-3 | 禁用 `std::fs::read_dir` | 返回顺序由文件系统决定。用 `pjsk_core::fs::read_dir_sorted` | disallowed-methods |
| D-4 | 禁用 `std` 的超越函数（`sin/cos/tan/exp/ln/powf`…） | `std` 调用系统 libm，macOS 与 Windows 实现不同 → 结果不同。用 `pjsk_core::det_math`（基于 `libm` crate，纯 Rust 实现，跨平台一致） | disallowed-methods |
| D-5 | 所有 profile 的优化行为一致 | 避免 debug 与 release 产出不同像素 | workspace `Cargo.toml` profile 统一 |
| D-6 | `unsafe` 仅允许出现在 `pjsk-live2d` 的 FFI 层 | 其余地方的 `unsafe` 容易引入未定义行为 → 不可复现 | workspace lint `unsafe_code = "warn"` + 评审 |

---

## R 级（人工评审）

### 着色器

| ID | 规则 | 理由 |
|---|---|---|
| R-1 | **关闭 fast-math** | Metal 着色器编译器**默认开启** fast-math。不关就已经不确定了。<br>⚠️ 需确认 wgpu 是否暴露 `MTLCompileOptions.fastMathEnabled`，未暴露可能要 patch（见 `risks.md` R4） |
| R-2 | 着色器内禁用超越函数 | `pow/exp/log/sin/cos/rsqrt/normalize/inversesqrt` 的 ULP 容差各厂商自定。<br>归一化写 `v / sqrt(dot(v,v))`——`sqrt` 与 `/` 是 IEEE-754 保证正确舍入的，`rsqrt` 不是 |
| R-3 | 全程 `f32`，禁 `f16` | `half` 在 Apple GPU 上是真 fp16，在 NVIDIA 上可能被提升到 fp32 |
| R-4 | FMA 显式化 | 编译器是否把 `a*b+c` 合并成 fma 由后端自行决定，会改变舍入。要么处处显式 `fma()`，要么用临时变量阻止合并 |

### 管线

| ID | 规则 | 理由 |
|---|---|---|
| R-5 | **禁用 MSAA**，改 2×/4× SSAA + 自写固定权重 box 降采样 | MSAA 的采样点位置由厂商自定义，Apple 与 NVIDIA 不同，必然不一致 |
| R-6 | 禁用各向异性过滤 | 各厂商实现完全不同 |
| R-7 | 不用 GPU 自动生成 mipmap | 滤波算法与 LOD 精度不同。离线预生成 mip 链显式上传 |
| R-8 | UI 层手动双线性 | `textureLoad` 取 4 texel 自己插值，绕开硬件过滤器的子纹素精度差异（D3D 只要求「至少 8 位」）。UI 多为整数倍缩放，成本极低 |
| R-9 | Render target 用 `Rgba8Unorm` / `Rgba16Unorm`，不用 fp16 RT | 8-bit UNORM 的 blend 规范较严，跨厂商基本一致 |

### CPU 侧

| ID | 规则 | 理由 |
|---|---|---|
| R-10 | 浮点累加顺序固定 | 浮点加法不满足结合律 |
| R-11 | 多线程 reduce 按固定次序合并 | 不能「谁先完成谁先加」 |
| R-12 | 并行渲染的分片边界不影响结果 | Pass 2 无状态是前提；若发现分片会改变结果，说明有隐藏状态 |

---

## 已知无法消除的差异

以下项目**规范上就不保证跨厂商一致**，已接受并交由容差吸收：

- 三角形边缘覆盖判定的子像素精度位数
- 硬件纹理过滤（Live2D 网格层仍使用，仅 UI 层手动实现）
- 驱动更新可能改变实现

这也是为什么 `spec/tolerance.md` 的主判据是**差异形态**（必须落在几何边缘 ±1px 带内）而非幅度——上述差异天然只出现在边缘。**任何出现在填充区内部的差异都是 bug，不是浮点误差。**

## 回归方式

T1 的验收载体是逐帧 BLAKE3 哈希。两端跑同一份参数表，比对帧哈希；不一致则进入 `pjsk-fidelity` 的形态分类流程。

⚠️ **CI 限制**：GitHub 托管 runner 没有 NVIDIA 独显，**T1 无法在托管 CI 上跑真实 GPU 比对**。需要自建 runner 或约定本地跑。见 `testing.md`。
