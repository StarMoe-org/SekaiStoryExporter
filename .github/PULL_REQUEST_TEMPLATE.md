<!-- 中英皆可 / Write in Chinese or English, whichever is clearer for you. -->

## 变更内容 / What changed

<!-- 一句话说明做了什么 / One sentence -->

## 关联 / Links

- 决策 / ADR:
- Issue:

## 检查清单 / Checklist

- [ ] `cargo fmt --all --check` 通过 / passes
- [ ] `cargo clippy --workspace --all-targets` 通过 / passes
- [ ] `cargo test --workspace` 通过 / passes
- [ ] 新增术语已进术语表 / new terms added to `docs/spec/glossary.md`
- [ ] 坐标量使用带空间标记的类型，未手写 Y 轴翻转 /
      coordinate values use space-tagged types; no hand-written Y-flips

> CI 会在 macOS、Windows、Linux 上运行以上检查 / CI runs these on macOS, Windows and Linux.

### 若触碰渲染路径 / If you touched the render path

`sse-render` · `sse-bake` · `sse-live2d` · `sse-ugui` · `sse-text` · `sse-core`

- [ ] 附改动前后同一批帧的比对 / attach a before/after comparison of the same frames
- [ ] 符合确定性 R 级规则 / complies with the R-level rules in
      `docs/conventions/determinism.md`
- [ ] 输出像素有变化则已记入 `CHANGELOG.md` / pixel changes recorded in `CHANGELOG.md`

### 若变更架构决策 / If you changed an architecture decision

- [ ] 新增 ADR，并把被取代的 ADR 标为 superseded /
      added an ADR under `docs/adr/` and marked the replaced one as superseded
