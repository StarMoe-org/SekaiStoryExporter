# Changelog

格式参考 [Keep a Changelog](https://keepachangelog.com/)。

**本项目的特殊约定**：任何会改变输出像素的变更，必须在此标注并注明 fidelity 影响范围。
**Project-specific rule**: any change that alters output pixels must be recorded here
together with its fidelity impact.

## [Unreleased]

### 变更 / Changed
- 项目改名为 **SekaiStoryExporter**（缩写 `sse`）：13 个 crate 由 `pjsk-*` 改为 `sse-*`，
  Rust 路径 `pjsk_core` → `sse_core`，CLI 子命令写作 `sse <cmd>`。无像素影响（决策 Q23）
- Renamed the project to **SekaiStoryExporter** (`sse`): crates `pjsk-*` → `sse-*`. No pixel impact.

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
- Round 6（Q24–Q31）：Live2D 改为 Unity 语义（取代 Q8）；资产只来自 SekaiStoryRipper 输出，
  `ripper-format` 以 git tag 依赖；口型同步解挂；Q28 分辨率定义待 PlayCover 实测；
  M1 = CPU 确定性链路，M2 = 静态首帧。无像素影响（尚无渲染实现）
- Q28 定为严格原生分辨率、无开关：逆向更正 [640,1080] 钳位只作用于 Live 画质档，剧情后备缓冲 = 原生分辨率，
  角色层固定 2304×1536 RT 缩放合成。**将决定未来的像素输出**（尚无渲染实现，当前无影响）
- Round 7（Q32–Q39）：IR v1 定稿（`docs/spec/ir.md`）——薄 IR、按帧模拟、按游戏帧率模拟（数值待逆向）、
  Unsupported 参与调度、保留分支树（仅 Action=5）、调试 JSON、无人工覆盖、玩家名默认「「世界」的居民」
- Q20 二进制 fixture = Git LFS
- Q21 CI = 暂不启用，仅手动触发
- Q22 项目语言 = 代码与 commit 英文 / PR·Issue 中英皆可 / 设计文档中文 / 门面双语

### 待定 / Open
- `docs/spec/ir.md`、`docs/spec/param-table.md` 尚未设计
- T1（跨平台帧哈希比对）需自建带 NVIDIA 卡的 runner；当前靠 PR 附本地报告
