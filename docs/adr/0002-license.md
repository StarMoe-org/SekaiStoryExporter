# ADR-0002: 开源许可证 / Open-source licence

- **状态 / Status**：accepted
- **日期 / Date**：2026-09-22
- **相关 / Related**：ADR-0003、ADR-0006

## 决策 / Decision

**AGPL-3.0-or-later**，外加一条针对 Live2D Cubism Core 的链接例外（见下）。

`LICENSE` 为 GNU AGPL v3 官方全文；`Cargo.toml` 的 `license` 字段为 `AGPL-3.0-or-later`。

## 背景 / Context

项目开源代码，资产由用户自备（ADR-0006）。项目不分发游戏资产，也不分发 Live2D Cubism SDK 二进制。

## ⚠️ AGPL 与 Cubism Core 的兼容性问题

这是本 ADR 的核心。

Live2D Cubism Core 是**闭源静态库**，其许可证与 (A)GPL 不兼容。AGPL-3.0 要求：分发「组合作品」时，必须以 AGPL 提供**全部对应源码**。Cubism Core 无法满足这一点，且它不属于 GPL 的「系统库」例外。

因此：

| 场景 | 是否有问题 |
|---|---|
| 分发源码，或分发**不含** Cubism Core 的编译产物（本项目的做法，见 ADR-0003：Core 在运行时由用户提供并加载） | ✅ 发布包里没有 Core；运行时与 Core 组合由下面的附加许可覆盖 |
| 任何人把 Cubism Core **打包进**编译产物一起分发 | ⚠️ 需依赖下面的附加许可，并且还须遵守 Live2D 自身的再分发条款 |

### 采取的措施：附加许可（GPL §7 additional permission）

在 `LICENSE` 之外增加一份 `LICENSE-EXCEPTION`，作为 AGPL-3.0 第 7 条允许的附加许可：

> 作为额外许可，版权持有者授权你将本程序与 Live2D Cubism Core（或其衍生库）链接并分发由此产生的可执行文件，而不因此要求 Live2D Cubism Core 本身遵循 AGPL 的条款。

- 该例外**只能由版权持有者授予**。若将来接受外部贡献，贡献者须同意其代码同样适用此例外（写进 `CONTRIBUTING.md`）。
- 该例外**不豁免** Live2D 自身的授权条款——使用者仍须自行取得 Cubism SDK 并遵守其许可。

### AGPL §13（网络条款）

若有人将本工具作为网络服务提供（例如在线剧情渲染站点），其用户有权取得对应源码。这符合项目意图，无需额外处理，但需在 README 中说明。

## 后果 / Consequences

- 正面：衍生的在线服务也必须开源；与「资产不分发」的定位一致
- 负面：AGPL 会劝退部分商业集成方——这与项目的非商用定位一致，视为可接受
- 贡献者须同意其代码同样适用链接例外（见 `CONTRIBUTING.md`）

## 复审条件 / Review triggers

- Live2D 变更 Cubism Core 的授权条款
- 计划把 Cubism Core 打包进发布物
