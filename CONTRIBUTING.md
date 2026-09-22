# Contributing / 贡献指南

> Language policy: code and Git history in English, design docs in Chinese, entry
> points bilingual. See [`docs/conventions/language.md`](docs/conventions/language.md).
> 语言规约：代码与 Git 历史用英文，设计文档用中文，门面文档双语。

---

## English

### Before you start

**This repository contains no game assets.** You must supply them yourself -- see `README.md`.

Environment:

| Dependency | Purpose | Required |
|---|---|---|
| Rust | pinned by `rust-toolchain.toml`; `rustup` switches automatically | yes |
| ffmpeg | encoding and mixing | yes |
| Git LFS | binary fixtures (`git lfs install` after cloning) | yes |
| Live2D Cubism Core | obtain it yourself from Live2D Inc. | yes |
| .NET | generating L1 oracle fixtures | dev only |
| PlayCover + the game | capturing ground truth | fidelity work only |

### Read these three first

1. [`docs/decisions.md`](docs/decisions.md) -- the architecture decisions. **Single source of truth.**
2. [`docs/spec/glossary.md`](docs/spec/glossary.md) -- terminology. Read before writing code.
3. [`docs/spec/coordinate-systems.md`](docs/spec/coordinate-systems.md) -- seven coordinate
   spaces and the time base. The most frequent source of bugs in this project.

### Branches and commits

- `main` is protected; everything goes through a PR.
- Branch names: `feat/<scope>-<short>`, `fix/...`, `docs/...`, `re/...` (reverse engineering).
- Conventional Commits, **scope is the crate name**, message in English:

  ```
  feat(pjsk-ugui): implement CanvasScaler MatchWidthOrHeight mode
  fix(pjsk-render): correct Y-flip in UI bilinear sampling
  re(constants): add full SpecialEffectType enum (cn-5.2.0)
  ```

### CI is not enabled

All checks run locally. Before pushing:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo test --workspace
```

### PR checklist

General:

- [ ] fmt / clippy / tests pass locally
- [ ] New terms added to `docs/spec/glossary.md`
- [ ] Coordinate values use space-tagged types; no hand-written Y-flips

If you touch the render path (`pjsk-render`, `pjsk-bake`, `pjsk-live2d`, `pjsk-ugui`,
`pjsk-text`, `pjsk-core`):

- [ ] Attach a **local T1 frame-hash report**
- [ ] Comply with the R-level rules in `docs/conventions/determinism.md`
- [ ] If output pixels change, state the scope in the PR and record it in `CHANGELOG.md`

If you add reverse-engineered findings:

- [ ] It goes into `constants.yaml`, not hard-coded Rust
- [ ] Provenance is complete (source / method / confidence)
- [ ] The matching entry in `open-questions.md` is ticked
- [ ] No raw dumps, game source, or assets committed

If you change an architecture decision:

- [ ] Add an ADR under `docs/adr/`
- [ ] **Append** a revision entry to `docs/decisions.md` (never overwrite in place)

### Review priorities

Ordered by actual risk in this project:

1. **Determinism** -- any new source of non-determinism (see determinism R-level rules)
2. **Coordinate spaces** -- computing in the wrong space
3. **Dependency direction** -- does `pjsk-render` accidentally depend on `pjsk-timeline` /
   `pjsk-scenario`? That breaks Pass 2 statelessness
4. **Provenance** of reverse-engineered numbers
5. Ordinary correctness and readability

### Not accepted

- Trading determinism for performance, unless justified by an ADR
- Hard-coding reverse-engineered constants into Rust
- Any game asset in the repository
- Dependencies that require compiling C/C++ sources (decision Q1)

### Licence of contributions

By contributing you agree that your code is licensed under **AGPL-3.0-or-later**
**and** that it is covered by the Live2D Cubism Core linking exception in
`LICENSE-EXCEPTION`. Without the latter, the project could never distribute a
working binary. See [ADR-0002](docs/adr/0002-license.md).

---

## 中文

### 开始之前

**本仓库不含任何游戏资产**，需自行提供，见 `README.md`。环境依赖见上方英文表格。
克隆后需执行一次 `git lfs install`。

### 先读这三份

1. [`docs/decisions.md`](docs/decisions.md) — 架构决策，**单一事实来源**
2. [`docs/spec/glossary.md`](docs/spec/glossary.md) — 术语表，写代码前必读
3. [`docs/spec/coordinate-systems.md`](docs/spec/coordinate-systems.md) — 坐标系与时间基，本项目最高频 bug 来源

### 分支与提交

`main` 受保护，一律走 PR。分支名 `feat/<scope>-<短描述>` 等。
Commit 遵循 Conventional Commits，**scope 用 crate 名，正文用英文**（见语言规约）。

### CI 未启用

所有检查本地执行，提交前跑 fmt / clippy / test 三件套。

### PR 检查清单

与英文版一致，要点：

- 通用：fmt/clippy/test 通过；新术语进术语表；坐标量用带空间标记的类型，不手写 Y 轴翻转
- **动渲染路径**：附本地 T1 帧哈希报告；符合确定性 R 级规则；像素有变化则记入 CHANGELOG
- **新增逆向结论**：进 `constants.yaml` 而非硬编码；provenance 填齐；`open-questions.md` 打勾；不提交原始 dump
- **变更架构决策**：新增 ADR；`decisions.md` **追加**修订条目而非原地覆盖

### 评审重点

按本项目实际风险排序：确定性 → 坐标系 → 依赖方向（`pjsk-render` 不得依赖 `pjsk-timeline`/`pjsk-scenario`）→ 逆向数据的 provenance → 常规正确性。

### 不接受的改动

为性能牺牲确定性（除非有 ADR 论证）、把逆向常量硬编码进 Rust、把游戏资产加进仓库、
引入需要编译 C/C++ 源码的依赖（决策 Q1）。

### 贡献的许可

提交代码即表示你同意其以 **AGPL-3.0-or-later** 授权，**并且**同意适用
`LICENSE-EXCEPTION` 中的 Live2D Cubism Core 链接例外。缺少后者，项目将永远无法分发可用的二进制。
详见 [ADR-0002](docs/adr/0002-license.md)。
