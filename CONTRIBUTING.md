# 贡献指南

## 开始之前

**本仓库不含任何游戏资产。** 运行需自行提供，见 `README.md`。

环境：
- Rust（版本由 `rust-toolchain.toml` 钉死，`rustup` 会自动切换）
- ffmpeg（导出）
- .NET（仅在生成 L1 oracle fixture 时需要）
- macOS + PlayCover（仅在采集 ground truth 时需要）

## 先读这三份

新人按顺序：

1. `docs/decisions.md` — 18 项架构决策，**单一事实来源**
2. `docs/spec/glossary.md` — 术语表，写代码前必读
3. `docs/spec/coordinate-systems.md` — 坐标系规约，本项目最高频 bug 来源

## 分支与提交

- `main` 受保护，一律走 PR
- 分支名：`feat/<scope>-<短描述>`、`fix/...`、`docs/...`、`re/...`（逆向）
- Commit 遵循 Conventional Commits，**scope 用 crate 名**：

  ```
  feat(pjsk-ugui): 实现 CanvasScaler 的 MatchWidthOrHeight 模式
  fix(pjsk-render): 修正 UI 层双线性采样的 Y 轴翻转
  re(constants): 补 SpecialEffectType 完整枚举（cn-5.2.0）
  ```

## PR 检查清单

通用：

- [ ] `cargo fmt --check` / `cargo clippy` 通过
- [ ] L0 / L1 测试通过
- [ ] 新增术语已进 `docs/spec/glossary.md`
- [ ] 坐标量使用了带空间标记的类型，未手写 Y 轴翻转

**触碰渲染路径**（`pjsk-render` / `pjsk-bake` / `pjsk-live2d` / `pjsk-ugui` / `pjsk-text` / `pjsk-core`）时额外：

- [ ] 附**本地 T1 帧哈希报告**（托管 CI 跑不了，见 `docs/testing.md`）
- [ ] 符合 `docs/conventions/determinism.md` 的 R 级规则
- [ ] 若输出像素有变化，在 PR 描述中说明变化范围，并在 `CHANGELOG.md` 标注

**新增逆向结论**时额外：

- [ ] 进的是 `constants.yaml` 而非硬编码
- [ ] provenance 填齐（source / method / confidence）
- [ ] 相关 `open-questions.md` 条目已打勾
- [ ] 未提交原始 dump、游戏源码或资产

**变更架构决策**时额外：

- [ ] 新增一份 ADR（`docs/adr/`）
- [ ] `docs/decisions.md` 总表已更新（**追加修订条目，不原地覆盖**）

## 评审重点

按本项目的实际风险排序，评审时优先看：

1. **确定性**：有没有引入不确定来源（见 determinism R 级清单）
2. **坐标系**：有没有在错误的空间里计算
3. **依赖方向**：`pjsk-render` 是否意外依赖了 `pjsk-timeline` / `pjsk-scenario`（这会破坏 Pass 2 无状态性）
4. **逆向数据的 provenance**：数字是哪来的
5. 常规的正确性与可读性

## 不接受的改动

- 为了性能牺牲确定性，除非附带 ADR 论证
- 把逆向常量硬编码进 Rust
- 在仓库中加入任何游戏资产
- 引入需要编译 C/C++ 源码的依赖（决策 Q1）
