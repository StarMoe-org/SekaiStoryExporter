# ADR-0006: 资产来源：Ripper 输出 + 用户客户端导出的 UI 套件

- **状态**：accepted
- **日期**：2026-09-24；UI 套件 2026-09-25；格式核对 2026-09-26

## 背景

- 剧情用到的模型、动作、语音、背景等在游戏 CDN 上；对话框、转场贴图、字体和转场粒子 prefab 则随客户端安装包分发。
- 这些资产的版权归权利方所有，本项目不能分发。

## 决策

- 开源代码，**不分发任何游戏资产**。缺资产时明确报错，并列出缺失清单，不静默降级。
- CDN 资源：sse **只读取 SekaiStoryRipper 的输出**（`library/` + `episodes/`），自身不含游戏 CDN 下载或解密代码，也不接触密钥。Ripper 的输出可以在本地目录，也可以在 S3（ADR-0015）。
- **接口就是存储**（与 SekaiStoryRipper 的 ADR-0013 相同）：sse 与 Ripper 之间只有 library（本地目录或 S3 前缀，布局相同）和 `ripper-format` 这个类型 crate，不链接 Ripper 的任何实现。
- 格式类型来自 `ripper-format`，以 git 依赖固定到某个 tag（`sse_assets::RIPPER_RELEASE`，测试保证与 `Cargo.toml` 一致）。升级 tag 是一次显式变更，要写进 CHANGELOG。
- **打开 library 时先核对 `ripper.lock.json`**：它必须是 `ripper-lock` 文档，`formats` 表里 sse 读取的每个格式都要与编译时的 `ripper_format::formats()` 版本相同；表里多出 sse 不认识的格式可以忽略。不一致时报错并提示用对应版本的 Ripper 重新导出。
- 逐个文档读取时仍检查各自的 `format` / `version`：剧集索引、`sse-motion`、`_ripper.json`（`ripper-unpack`）、`_objects.json`（`ripper-objects`）。特效 bundle 的格式错误是硬错误，不会被当成“加载不了的特效”跳过。
- 原始载荷（typetree JSON、PNG、WAV、moc3、`model3.json`）的结构由游戏决定，不在格式版本之内。
- 客户端内置资源：由用户用 [`tools/ui-kit/extract.py`](../../tools/ui-kit/extract.py) 从自己的客户端（`.ipa`、`.app`、`Data` 目录或 `data.unity3d`）导出成 UI 套件，通过 `--ui` 传入。套件包括对话框相关 sprite、转场贴图、客户端字体，以及转场粒子参数（`fx_transition_scenario.json`，`sse-fx` v1）。
- 套件中缺少某个文件时，只是不画对应的元素，并写入导出报告。

## 后果

- 正面：仓库和发布包里没有任何受版权保护的数据；UI 与游戏使用同一份素材。
- 负面：使用者需要持有游戏客户端，并多运行一步导出脚本。

## 复审条件

权利方对工具提出要求，或客户端的打包方式发生变化时。
