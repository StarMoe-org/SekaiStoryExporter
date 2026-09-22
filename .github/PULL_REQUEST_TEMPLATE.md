## 变更内容

<!-- 一句话说明做了什么 -->

## 关联

- 决策 / ADR：
- open-questions 条目：

## 检查清单

- [ ] `cargo fmt --check` / `cargo clippy` 通过
- [ ] L0 / L1 测试通过
- [ ] 新增术语已进 `docs/spec/glossary.md`
- [ ] 坐标量使用带空间标记的类型，未手写 Y 轴翻转

### 若触碰渲染路径

- [ ] 附本地 **T1 帧哈希报告**
- [ ] 符合 `docs/conventions/determinism.md` R 级规则
- [ ] 输出像素有变化 → 已在 `CHANGELOG.md` 标注影响范围

### 若新增逆向结论

- [ ] 进的是 `constants.yaml`，未硬编码
- [ ] provenance 填齐（source / method / confidence）
- [ ] 未提交原始 dump / 游戏源码 / 资产

### 若变更架构决策

- [ ] 新增 ADR
- [ ] `docs/decisions.md` 已**追加**修订条目（未原地覆盖）
