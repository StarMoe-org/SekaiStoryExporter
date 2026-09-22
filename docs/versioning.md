# 版本策略

> 决策 Q16：版本锁定 + 手动适配。Q18：长期项目，正确性优先。

## 三个版本号

| | 含义 | 位置 |
|---|---|---|
| 项目版本 | 本工具自身的版本 | `Cargo.toml` workspace version |
| **游戏版本** | 被支持的 PJSK 客户端版本（分区服） | `docs/reverse/versions/<region>-<ver>/` |
| 资产版本 | 抓取到的资产快照 | `assets.lock` |

三者**不联动**。项目版本变化不代表支持的游戏版本变化，反之亦然。

## 支持矩阵

`main` 分支在任一时刻只承诺支持**一组**（region, game_version）。当前支持的组合写在 `README.md`，并在 CLI `--version` 输出中体现。

需要同时支持多个版本时的做法：**保留多份 `constants.yaml`，代码只有一套**，通过 `--game-version` 选择数据集。这与「手动适配」不冲突——手动的是分析过程，不是切换机制。

```
docs/reverse/versions/
├── cn-5.2.0/
│   ├── constants.yaml
│   ├── enums.yaml
│   └── layout/            # UI dumper 产出的布局基线
└── jp-6.0.1/
    └── ...
```

## 游戏更新适配 checklist

游戏更新后按顺序执行。**不要跳步**——跳过的步骤会在三个月后以「某个分辨率下对话框偏了 2px」的形式回来。

- [ ] 1. 记录新的游戏版本号与资产版本号
- [ ] 2. 重跑 `tools/fetch`，比对 `assets.lock` 差异，列出新增/变更资产
- [ ] 3. 重跑 `tools/dumper` 的 UI 层次导出，与上一版布局基线做**结构化 diff**（不是看截图）
- [ ] 4. 重跑全量剧本统计，检查是否出现新的 `SpecialEffectType` 或新的 Action 值
- [ ] 5. 复核 `constants.yaml` 中 `confidence: low` 的条目——它们最可能失效
- [ ] 6. 检查 Cubism SDK 版本 / moc3 version byte 是否变化
- [ ] 7. 复核色彩空间、CanvasScaler 设置、Reference Resolution 是否变化
- [ ] 8. 跑 L0/L1 测试；跑 T2 帧级回归
- [ ] 9. 新建 `versions/<region>-<new_ver>/`，更新 README 支持矩阵
- [ ] 10. 在 `CHANGELOG.md` 记录本次适配的发现

第 3 步和第 5 步是回报最高的两步。

## 工具链版本

`rust-toolchain.toml` 钉死版本。升级工具链视为**可能改变浮点与优化行为的变更**：

- 必须走 ADR
- 必须重跑 T1 全量帧哈希回归
- 不得与其他变更混在同一个 PR

## 破坏性变更

以下情况允许破坏兼容：

- IR / ParamTable 格式变更（两者都带版本号字段，旧文件应给出明确的「版本不匹配」错误而非乱解析）
- 坐标系或时间基约定变更（极高成本，需 ADR）

以下情况**不允许**静默变更：

- 任何会改变输出像素的修改，必须在 CHANGELOG 中标注，并附 fidelity 报告
