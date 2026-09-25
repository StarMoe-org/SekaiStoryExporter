# ADR-0013: 项目语言与命名

- **状态**：accepted
- **日期**：2026-09-22（语言）；2026-09-23（命名）

## 决策

- 代码标识符、doc comment、commit message 用英文；PR 和 Issue 中英文皆可；设计文档以中文为主；README 等门面文档双语。细则见 [`conventions/language.md`](../conventions/language.md)。
- 项目名为 **SekaiStoryExporter**，缩写 **sse**：crate 前缀 `sse-*`，可执行文件 `sse`。`PJSK` 只用来指代游戏本身，不出现在标识符中。

## 复审条件

主要贡献者群体发生变化时。
