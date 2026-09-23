<!-- 中英皆可 / Write in Chinese or English, whichever is clearer for you. -->

## 变更内容 / What changed

<!-- 一句话说明做了什么 / One sentence -->

## 关联 / Links

- 决策 / ADR:
- open-questions 条目 / entry:

## 检查清单 / Checklist

- [ ] `cargo fmt --all --check` 通过 / passes
- [ ] `cargo clippy --workspace --all-targets` 通过 / passes
- [ ] `cargo test --workspace` 通过 / passes
- [ ] 新增术语已进术语表 / new terms added to `docs/spec/glossary.md`
- [ ] 坐标量使用带空间标记的类型，未手写 Y 轴翻转 /
      coordinate values use space-tagged types; no hand-written Y-flips

> CI 未启用，以上均为本地执行 / CI is not enabled; run these locally.

### 若触碰渲染路径 / If you touched the render path

`sse-render` · `sse-bake` · `sse-live2d` · `sse-ugui` · `sse-text` · `sse-core`

- [ ] 附本地 **T1 帧哈希报告** / attach a local **T1 frame-hash report**
- [ ] 符合确定性 R 级规则 / complies with the R-level rules in
      `docs/conventions/determinism.md`
- [ ] 输出像素有变化则已记入 `CHANGELOG.md` / pixel changes recorded in `CHANGELOG.md`

### 若新增逆向结论 / If you added reverse-engineered findings

- [ ] 进的是 `constants.yaml`，未硬编码 / goes into `constants.yaml`, not hard-coded Rust
- [ ] provenance 填齐（source / method / confidence） / provenance complete
- [ ] `open-questions.md` 已打勾 / matching entry ticked
- [ ] 未提交原始 dump / 游戏源码 / 资产 / no raw dumps, game source, or assets

### 若变更架构决策 / If you changed an architecture decision

- [ ] 新增 ADR / added an ADR under `docs/adr/`
- [ ] `docs/decisions.md` 已**追加**修订条目（未原地覆盖） /
      **appended** a revision entry (never overwrite in place)
