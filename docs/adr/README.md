# 架构决策记录（ADR）

本目录记录 SekaiStoryExporter 的架构决策，每条决策一个文件，编号递增、不复用。
决策变更时新建一篇 ADR，并把旧 ADR 的状态改为 `superseded by ADR-NNNN`。代码注释引用决策时写 `ADR-NNNN`。

| 编号 | 标题 | 状态 |
|---|---|---|
| [0001](0001-use-adr.md) | 用 ADR 记录架构决策 | accepted |
| [0002](0002-license.md) | 许可证：AGPL-3.0-or-later + Cubism Core 链接例外 | accepted |
| [0003](0003-rust-and-cubism-core.md) | 宿主语言 Rust；Cubism Core 运行时加载 | accepted |
| [0004](0004-two-pass-export.md) | 两阶段导出与无状态渲染 | accepted |
| [0005](0005-gpu-and-tolerance.md) | GPU 后端、平台与保真度容差 | accepted |
| [0006](0006-assets.md) | 资产来源：Ripper 输出 + 用户客户端导出的 UI 套件 | accepted |
| [0007](0007-live2d-semantics.md) | Live2D 语义：Unity 侧行为 | accepted |
| [0008](0008-ir.md) | 中间表示（IR） | accepted |
| [0009](0009-timeline.md) | 时间轴：按帧模拟，60 fps | accepted |
| [0010](0010-resolution.md) | 分辨率：严格原生 | accepted |
| [0011](0011-scope-and-text.md) | 内容范围与文字渲染 | accepted |
| [0012](0012-versioning.md) | 版本策略 | accepted |
| [0013](0013-language-and-naming.md) | 项目语言与命名 | accepted |
| [0014](0014-ci-and-release.md) | CI、测试数据与发布 | accepted |

模板见 [`0000-template.md`](0000-template.md)。
