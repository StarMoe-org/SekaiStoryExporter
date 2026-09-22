# Changelog

格式参考 [Keep a Changelog](https://keepachangelog.com/)。

**本项目的特殊约定**：任何会改变输出像素的变更，必须在此标注并注明 fidelity 影响范围。

## [Unreleased]

### 新增
- 项目骨架：Cargo workspace（13 crate + xtask）、工具链钉版本、确定性 lint 防线
- 架构决策记录（Q1–Q18）、风险登记、容差规范
- 规约文档：确定性、坐标系与时间基、术语表、逆向工作流、版本策略、测试策略
- 协作文档：CONTRIBUTING、ADR 模板与 ADR-0001/0002

### 待定
- LICENSE（见 ADR-0002）
- 二进制 fixture 存储方案（见 docs/testing.md）
- T1 在 CI 中的执行方案（无托管 NVIDIA runner）
