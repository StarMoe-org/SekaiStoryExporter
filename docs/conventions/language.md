# 语言规约 / Language policy

> 决策 2026-09-22：项目语言为 中文 / English。以下是其具体划分。
> Decision 2026-09-22: the project is bilingual (Chinese / English). This document defines the split.

## 划分 / The split

| 场合 / Context | 语言 / Language | 理由 / Rationale |
|---|---|---|
| 代码标识符、doc comment、错误信息、日志、测试名 | **English** | 代码必须对任何贡献者可读；标识符混用中文会破坏工具链 |
| Commit message | **English** | Git 历史是机器与人共同消费的长期记录，需单一语言 |
| PR、Issue、评审意见 | **双语接受 / Either** | 讨论的门槛应当低。用你能把话说清楚的那种语言 |
| `README.md`、`CONTRIBUTING.md`、`LICENSE-EXCEPTION`、PR/Issue 模板 | **双语 / Bilingual** | 项目门面与协作规则，两侧读者都需要 |
| `docs/` 下的设计与规约文档 | **中文为主 / Chinese-first** | 设计文档的价值在于细节与推理精度；强制翻译会牺牲深度。欢迎补英文版，但不作为门槛 |
| `docs/spec/glossary.md` | **中 / 英 / 日 对照** | 术语表本身就是翻译层 |

## 为什么 commit 是英文而 PR 不是 / Why commits are English but PRs are not

Commit message 会被 `git log`、`git blame`、CHANGELOG 生成、自动化工具反复消费，
且无法事后修改。它需要单一语言。

PR 与 Issue 是**讨论**，价值在于把问题说清楚。强制第二语言只会降低讨论质量，
或者让人干脆不提 issue。

Commit messages are consumed repeatedly by `git log`, `git blame`, changelog tooling,
and cannot be rewritten later, so they need one language. PRs and issues are
*discussion* -- forcing a second language lowers the quality of the discussion, or
stops people from filing at all.

## 代码中的术语 / Terminology in code

代码标识符一律使用 `docs/spec/glossary.md` 的 **English** 列，不自创同义词。

Identifiers must use the **English** column of `docs/spec/glossary.md`. Do not invent synonyms.

```rust
// ✅
struct ParamTable { /* ... */ }
fn bake_scenario(timeline: &Timeline) -> ParamTable { /* ... */ }

// ❌ 表外术语 / term not in the glossary
struct FrameStateDump { /* ... */ }
```

## 注释 / Comments

Doc comment（`///` `//!`）用英文。行内注释解释**为什么**时可用中文，但如果该注释对理解代码是必要的，应当用英文。

Doc comments (`///`, `//!`) are English. Inline `//` comments explaining *why* may be Chinese, but if the comment is necessary to understand the code, write it in English.

## 面向文档的交叉引用 / Cross-references

代码注释引用中文文档时，直接写路径即可，不必翻译文档标题：

```rust
//! Determinism rules apply here. See `docs/conventions/determinism.md`.
```
