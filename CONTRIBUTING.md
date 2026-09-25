# Contributing / 贡献指南

> Language policy: code and commit messages in English; **pull requests and issues in
> either language**; design docs in Chinese; entry points bilingual.
> See [`docs/conventions/language.md`](docs/conventions/language.md).
>
> 语言规约：代码与 commit 用英文；**PR 与 Issue 中英皆可**；设计文档用中文；门面文档双语。

---

## English

### Before you start

**This repository contains no game assets.** You must supply them yourself -- see `README.md`.

Environment:

| Dependency | Purpose | Required |
|---|---|---|
| Rust | pinned by `rust-toolchain.toml`; `rustup` switches automatically | yes |
| ffmpeg | encoding and mixing | to export video |
| Live2D Cubism Core | loaded at run time; obtain it yourself from Live2D Inc. (not needed to build or test) | to render |
| uv (Python) | `tools/ui-kit/extract.py` and `tools/stats/` | to build a UI kit |

### Read these three first

1. [`docs/adr/`](docs/adr/) -- the architecture decisions.
2. [`docs/spec/glossary.md`](docs/spec/glossary.md) -- terminology. Read before writing code.
3. [`docs/spec/coordinate-systems.md`](docs/spec/coordinate-systems.md) -- the coordinate
   spaces and the time base. The most frequent source of bugs in this project.

### Branches and commits

- `main` is protected; everything goes through a PR.
- Branch names: `feat/<scope>-<short>`, `fix/...`, `docs/...`.
- Conventional Commits, **scope is the crate name**, message in English
  (PRs and issues may be written in either Chinese or English):

  ```
  feat(sse-ugui): implement CanvasScaler MatchWidthOrHeight mode
  fix(sse-render): correct Y-flip in UI bilinear sampling
  ```

### Checks

CI (GitHub Actions and Gitea Actions) runs these on macOS, Windows and Linux. Run them locally
before pushing:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

### PR checklist

General:

- [ ] fmt / clippy / tests pass
- [ ] New terms added to `docs/spec/glossary.md`
- [ ] Coordinate values use space-tagged types; no hand-written Y-flips

If you touch the render path (`sse-render`, `sse-bake`, `sse-live2d`, `sse-ugui`,
`sse-text`, `sse-core`):

- [ ] Attach a before/after comparison of the same frames (see `docs/testing.md`)
- [ ] Comply with the R-level rules in `docs/conventions/determinism.md`
- [ ] If output pixels change, state the scope in the PR and record it in `CHANGELOG.md`

If you change an architecture decision:

- [ ] Add an ADR under `docs/adr/` and mark the one it replaces as superseded

### Review priorities

Ordered by actual risk in this project:

1. **Determinism** -- any new source of non-determinism (see determinism R-level rules)
2. **Coordinate spaces** -- computing in the wrong space
3. **Dependency direction** -- does `sse-render` accidentally depend on `sse-timeline` /
   `sse-scenario`? That breaks Pass 2 statelessness
4. Ordinary correctness and readability

### Not accepted

- Trading determinism for performance, unless justified by an ADR
- Any game asset, or data extracted from the game client, in the repository
- Dependencies that require compiling C/C++ sources (ADR-0003)

### Licence of contributions

By contributing you agree that your code is licensed under **AGPL-3.0-or-later**
**and** that it is covered by the Live2D Cubism Core linking exception in
`LICENSE-EXCEPTION`. See [ADR-0002](docs/adr/0002-license.md).

---

## 中文

### 开始之前

**本仓库不含任何游戏资产**，需自行提供，见 `README.md`。环境依赖见上方英文表格。
编译和测试不需要 Cubism SDK；渲染时才需要（运行时加载，见 ADR-0003）。

### 先读这三份

1. [`docs/adr/`](docs/adr/) — 架构决策
2. [`docs/spec/glossary.md`](docs/spec/glossary.md) — 术语表，写代码前必读
3. [`docs/spec/coordinate-systems.md`](docs/spec/coordinate-systems.md) — 坐标系与时间基，本项目最高频 bug 来源

### 分支与提交

`main` 受保护，一律走 PR。分支名 `feat/<scope>-<短描述>` 等。
Commit 遵循 Conventional Commits，**scope 用 crate 名，正文用英文**。
**PR 与 Issue 的正文中英皆可**，用你能把问题说清楚的那种语言（见语言规约）。

### 检查

CI（GitHub Actions 与 Gitea Actions）在 macOS、Windows、Linux 上跑 fmt / clippy / test，提交前请先在本地跑一遍。

### PR 检查清单

与英文版一致，要点：

- 通用：fmt/clippy/test 通过；新术语进术语表；坐标量用带空间标记的类型，不手写 Y 轴翻转
- **动渲染路径**：附改动前后同一批帧的比对；符合确定性 R 级规则；像素有变化则记入 CHANGELOG
- **变更架构决策**：新增 ADR，并把被取代的 ADR 标为 superseded

### 评审重点

按本项目实际风险排序：确定性 → 坐标系 → 依赖方向（`sse-render` 不得依赖 `sse-timeline`/`sse-scenario`）→ 常规正确性。

### 不接受的改动

为性能牺牲确定性（除非有 ADR 论证）、把游戏资产或从客户端提取的数据加进仓库、
引入需要编译 C/C++ 源码的依赖（ADR-0003）。

### 贡献的许可

提交代码即表示你同意其以 **AGPL-3.0-or-later** 授权，**并且**同意适用
`LICENSE-EXCEPTION` 中的 Live2D Cubism Core 链接例外。详见 [ADR-0002](docs/adr/0002-license.md)。
