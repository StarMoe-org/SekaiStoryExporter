# Changelog

格式参考 [Keep a Changelog](https://keepachangelog.com/)。

**本项目的特殊约定**：任何会改变输出像素的变更，必须在此标注并注明 fidelity 影响范围。
**Project-specific rule**: any change that alters output pixels must be recorded here
together with its fidelity impact.

## [Unreleased]

### 新增 / Added
- 项目骨架：Cargo workspace（13 crate + xtask），仅含职责文档注释，无实现
- 确定性规约落地为 `clippy.toml` 的编译期 deny，**已实测 5 条规则全部触发**
- 工具链钉死 1.98.1 + 双平台 target
- 规约文档：确定性、坐标系与时间基、术语表、语言规约、逆向 provenance、版本策略、测试策略
- 协作设施：CONTRIBUTING（双语）、ADR 体系、PR 与 issue 模板
- 许可：AGPL-3.0-or-later + Live2D Cubism Core 链接例外
- Git LFS 管理二进制 fixture

### 决策 / Decisions
- Q19 许可证 = AGPL-3.0-or-later（+ 链接例外）
- Q20 二进制 fixture = Git LFS
- Q21 CI = 暂不启用，仅手动触发
- Q22 项目语言 = 代码与 commit 英文 / PR·Issue 中英皆可 / 设计文档中文 / 门面双语

### 待定 / Open
- `docs/spec/ir.md`、`docs/spec/param-table.md` 尚未设计
- T1（跨平台帧哈希比对）需自建带 NVIDIA 卡的 runner；当前靠 PR 附本地报告
