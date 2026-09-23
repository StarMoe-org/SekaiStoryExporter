# 坐标系与时间基规约

> 本项目至少同时存在 7 套空间坐标系和 3 种时间表示。
> 经验上，这类项目最高频的 bug 不是算错，而是**在错误的坐标系里算对了**。
> 本规约是强制的。

## 空间坐标系清单

| 规范名 | 原点 | Y 轴 | 单位 | 出现在 |
|---|---|---|---|---|
| `UnityWorld` | 场景原点 | ↑ 向上 | unit | 摄像机、背景层、Live2D 根节点 |
| `UnityScreen` | 屏幕**左下** | ↑ 向上 | px | UGUI 的 screen space；**所有逆向出的 UI 参数都在这个系** |
| `RectLocal` | 父 rect 的 anchor 参考点 | ↑ 向上 | px | `RectTransform` 的 offset/pivot 计算 |
| `ImagePixel` | 图像**左上** | ↓ 向下 | px | 输出帧、PNG、diff harness、回读后的缓冲 |
| `Ndc` | 视口中心 | ↑ 向上 | — | wgpu 裁剪空间，x,y ∈ [-1,1]，z ∈ [0,1] |
| `TexUv` | 纹理**左上** | ↓ 向下 | — | 采样坐标，∈ [0,1] |
| `L2dCanvas` | 模型画布中心（由 moc3 `CanvasInfo` 给出） | ↑ 向上 | 模型单位 | Cubism Core 输出的顶点 |

**注意四处 Y 轴翻转**：`UnityScreen ↔ ImagePixel`、`Ndc ↔ ImagePixel`、`TexUv ↔ Ndc`、`L2dCanvas ↔ ImagePixel`。

## 中枢坐标系

> **`UnityScreen` 是本项目的中枢坐标系。**

理由：所有从游戏逆向出来的参数（RectTransform 数值、CanvasScaler 结果、坐标表）天然就在这个系里。选它做中枢意味着**逆向数据可以零转换地直接使用**，减少一整类抄写错误。

规则：
1. 布局、时间轴、参数表中的一切位置量**一律以 `UnityScreen` 存储**。
2. 只在提交给 GPU 的最后一步转换到 `Ndc`，在写出帧的最后一步转换到 `ImagePixel`。
3. `L2dCanvas → UnityScreen` 的变换由 `sse-live2d` 负责并且只做一次。

## 强制约定

### 1. 类型层面隔离

坐标量必须使用带空间标记的 newtype，不得用裸 `f32` / `[f32; 2]` 传递：

```rust
// sse-core::space
pub struct Point<S: Space> { pub x: f32, pub y: f32, _s: PhantomData<S> }
pub struct Rect<S: Space>  { /* ... */ }

pub struct UnityScreen; pub struct ImagePixel; pub struct Ndc; /* ... */
```

编译器会挡住绝大多数混用。**这是本项目使用 Rust 的主要收益之一，不要绕过它。**

### 2. 转换只有一处实现

所有跨坐标系转换必须走 `sse_core::space::convert`。**禁止在业务代码里手写 `y = height - y`** ——这行代码在本项目里有四种不同的正确写法和无数种错误写法。

### 3. 命名带空间后缀

变量与函数名必须体现坐标系：`rect_screen`、`pos_ndc`、`uv_of()`、`to_image_px()`。
不允许出现 `pos`、`rect`、`p` 这类无空间信息的命名。

### 4. 像素中心约定

统一采用 **像素中心为半整数**：像素 `(0,0)` 的中心在 `(0.5, 0.5)`。
这与 D3D10+ / Metal / Vulkan 的光栅化约定一致。任何整数化操作必须显式说明用的是 floor / round / 像素中心。

---

## 时间基

| 表示 | 类型 | 地位 |
|---|---|---|
| **模拟帧号** | `SimFrame(u32)` | ⭐ **权威表示**。时间轴、参数表以它索引；步长 = 游戏帧率（Q34，数值待 open-question #40） |
| **导出帧号** | `FrameNo(u32)` | 渲染、哈希、diff 的索引；由模拟帧按游戏的帧同步策略映射（Q34，规则待 #40） |
| 秒 | `Seconds(f64)` | **派生量**。只能由帧号派生 |
| 音频采样 | `SampleNo(u64)` | 仅在混音阶段使用 |

规则：

1. **时间的唯一权威是帧号。** 秒数只能由帧号派生，**禁止反向**（用秒数算回帧号会引入舍入歧义）。
2. **`sse-timeline` 按帧模拟**（Q33）：每模拟帧 `elapsed += deltaTime`（f32，与 Unity 一致），以 `elapsed < duration` 决定是否再等一帧。
   秒 → 帧的量化由这一逐帧累加**自然产生**，不存在单独的「round 规则」。IR 中的秒量（`delay`、`duration`）原样保留，见 `spec/ir.md` §10。
   动作曲线在模拟帧内的采样时刻 = 该帧累计的 `Time.time` 语义值（f32），同样不做取整。
3. 渲染路径不得出现任何实时时钟（见 `conventions/determinism.md` D-2）。
4. 模拟帧率与导出 `fps` 都进入帧哈希的计算输入——**不同 fps 的输出不可互相比对**。

## 分辨率与缩放

- 参考分辨率、目标分辨率、SSAA 倍率三者必须显式区分，不得用同一个变量表达。
- SSAA 只允许整数倍（见 determinism R-5）。
- `CanvasScaler` 的 `scaleFactor` 是 `UnityScreen` 内部的缩放，**不是** SSAA 倍率，两者不可混用。
