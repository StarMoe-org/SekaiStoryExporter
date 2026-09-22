# ADR-0002: 开源许可证 / Open-source licence

- **状态 / Status**：accepted
- **日期 / Date**：2026-09-22
- **相关 / Related**：decisions.md Q17

## 决策 / Decision

**AGPL-3.0-or-later**，外加一条针对 Live2D Cubism Core 的链接例外（见下）。

`LICENSE` 为 GNU AGPL v3 官方全文；`Cargo.toml` 的 `license` 字段为 `AGPL-3.0-or-later`。

## 背景 / Context

决策 Q17 选择「开源代码，资产由用户自备」。项目不分发游戏资产，也不分发 Live2D Cubism SDK 二进制。

## ⚠️ AGPL 与 Cubism Core 的兼容性问题

这是本 ADR 的核心，**必须在首次对外发布二进制前解决**。

Live2D Cubism Core 是**闭源静态库**，其许可证与 (A)GPL 不兼容。AGPL-3.0 要求：分发「组合作品」时，必须以 AGPL 提供**全部对应源码**。Cubism Core 无法满足这一点，且它不属于 GPL 的「系统库」例外。

因此：

| 场景 | 是否有问题 |
|---|---|
| 本项目**只分发源码**，用户自行获取 Cubism Core 并自行编译 | ✅ 无问题。用户为自己编译不构成「分发」 |
| 任何人分发**已链接 Cubism Core 的编译产物** | ❌ 构成分发组合作品，与 AGPL 冲突 |

### 采取的措施：附加许可（GPL §7 additional permission）

在 `LICENSE` 之外增加一份 `LICENSE-EXCEPTION`，作为 AGPL-3.0 第 7 条允许的附加许可：

> 作为额外许可，版权持有者授权你将本程序与 Live2D Cubism Core（或其衍生库）链接并分发由此产生的可执行文件，而不因此要求 Live2D Cubism Core 本身遵循 AGPL 的条款。

- 该例外**只能由版权持有者授予**。若将来接受外部贡献，贡献者须同意其代码同样适用此例外（写进 `CONTRIBUTING.md`）。
- 该例外**不豁免** Live2D 自身的授权条款——使用者仍须自行取得 Cubism SDK 并遵守其许可。

### AGPL §13（网络条款）

若有人将本工具作为网络服务提供（例如在线剧情渲染站点），其用户有权取得对应源码。这符合项目意图，无需额外处理，但需在 README 中说明。

## 后果 / Consequences

- 正面：衍生的在线服务也必须开源；与「资产不分发」的定位一致
- 负面：AGPL 会劝退部分商业集成方——这与 Q17 的非商用定位一致，视为可接受
- 待办：贡献者协议中须包含对链接例外的同意条款

## 复审条件 / Review triggers

- Live2D 变更 Cubism Core 的授权条款
- 项目开始接受外部代码贡献（需先落实贡献者对例外条款的同意）
- 计划分发编译好的二进制（届时须确认例外条款文本已就位）
