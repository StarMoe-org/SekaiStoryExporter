# 测试策略

> 本项目的正确性不可能靠「跑一下看看」保证。像素级目标要求把验证拆成可归因的层次：
> **每一层失败时，都应该能立刻说出问题在哪个模块。**

## 五层

| 层 | 验证对象 | 数据来源 | 需要游戏？ | 跑在哪 | 预算 |
|---|---|---|---|---|---|
| **L0** 单元测试 | 布局求值、曲线求值、时间轴编译、坐标转换 | 手写用例 | ❌ | 每次 push，托管 CI | < 1 min |
| **L1** Oracle fixture | UGUI 布局、TMP 排版、ScenarioPlayer 状态机 | `.NET` 跑反编译 C# 产出的期望值 | ❌（fixture 已固化） | 每次 push，托管 CI | < 2 min |
| **L2** 参数级比对 | Pass 1 输出的参数表 | hook 游戏 dump 的逐帧参数 | ✅ 产 fixture 时需要 | 本地 / nightly | 分钟级 |
| **L3** 帧级 fidelity | 最终像素 | T1：两端自渲染<br>T2：PlayCover 原始帧 | T2 需要 | **自建 runner 或本地** | 分钟～小时 |
| **L4** 端到端冒烟 | 能否出片 | 一段短剧情 | ❌ | nightly | 分钟级 |

## 关键约束：CI 跑不了 T1

**GitHub 托管 runner 没有 NVIDIA 独显**，因此 T1（macOS vs Windows 的 GPU 渲染比对）**无法在托管 CI 上进行**。

**当前决策（2026-09-22）：CI 暂不启用。** `.github/workflows/ci.yml` 保留为配置，
仅允许 `workflow_dispatch` 手动触发。

因此目前**所有测试层都在本地执行**：

- 提交前自行跑 `cargo fmt --check`、`cargo clippy --workspace --all-targets`、`cargo test --workspace`
- **动渲染路径的 PR 必须附本地 T1 报告**（见 `CONTRIBUTING.md`）

将来启用 CI 时，把 ci.yml 中的 `push` / `pull_request` 触发器取消注释即可跑 L0/L1；
T1 仍需自建带 NVIDIA 卡的 runner。

## 为什么 L2 是关键一层

Live2D 的失败模式很难定位：画面不对，可能是曲线求值错、fade 权重错、Core 变形错、或渲染错。

参数表把中间状态显式化后：

- **参数表对上了** → 问题必在 Core 之后的渲染层
- **参数表对不上** → 问题在动作/物理/状态机

这把一个四维的 bug 空间切成两个二维的。**L2 的投入回报率高于 L3。**

## Fixture 管理

fixture 分两类，处理方式不同：

| 类型 | 例子 | 体积 | 存放 |
|---|---|---|---|
| 结构化 | UI 布局 dump、参数表片段、oracle 期望值 | 小（KB～MB），可 diff | 直接入库，JSON/YAML |
| 二进制 | 参考帧图像 | 大（MB/帧） | **Git LFS**（决策 2026-09-22） |

`.gitattributes` 已把 `tests/fixtures/**/*.{png,exr,bin,zst}` 纳入 LFS 管理；
`tests/fixtures/**/*.{json,yaml}` 保持普通文本以便 diff。

克隆后需执行一次 `git lfs install`（本仓库已配置 `--local` hooks）。

> **仍建议控制体积**：即便有 LFS，也优先只存**裁剪区域**（对话框区、角色区各一小块）
> 而非整帧，并以逐帧哈希作为日常判据。完整帧只在排查时本地重现。
> LFS 解决的是「能存」，不解决「该存多少」。

## 测试命名约定

```
L0:  crates/<crate>/src/**/tests.rs      或 tests/
L1:  crates/<crate>/tests/oracle_*.rs
L2:  crates/pjsk-bake/tests/param_*.rs
L3:  crates/pjsk-fidelity/tests/         + CLI 子命令 `pjsk diff`
L4:  tests/smoke/
```

## 不写什么测试

- 不为 wgpu、ffmpeg 等外部依赖写测试
- 不为「代码能跑」写测试——L0 应该测的是**语义**（这个 anchor 配置在 19.5:9 下应该落在哪），不是「函数不 panic」
