# ADR-0014: CI、测试数据与发布

- **状态**：accepted（取代早期「CI 暂不启用」「二进制 fixture 用 Git LFS」两项决策）
- **日期**：2026-09-25

## 背景

公开后需要让外部贡献者和维护者都能自动验证构建。仓库不能包含游戏资产，所以测试只能使用合成数据；需要真实资产的验证只能在本地进行。

## 决策

- CI 同时提供 GitHub Actions（`.github/workflows/`）和 Gitea Actions（`.gitea/workflows/`），在 macOS、Windows、Linux 上运行 fmt、clippy、test 和 release 构建。构建不需要 Cubism SDK（ADR-0003）。
- 推送 `v*` tag 时自动构建各平台的 `sse` 并发布 GitHub Release；发布包不含 Cubism Core。
- 仓库不使用 Git LFS，也不提交任何二进制 fixture。需要真实资产的测试标记为 `#[ignore]`，通过环境变量指向本地数据运行。

## 后果

- 正面：任何人都能在干净的机器上构建、测试和发版。
- 负面：像素级回归只能在本地、用真实资产运行。

## 复审条件

有了可合法分发的测试素材时。
