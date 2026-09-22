# ADR-0002: 开源许可证

- **状态**：⚠️ **proposed（待定，需项目所有者决策）**
- **日期**：2026-09-22
- **相关**：decisions.md Q17

## 背景

决策 Q17 选择「开源代码，资产由用户自备」。需要确定许可证。

约束：

1. Rust 生态惯例是 `MIT OR Apache-2.0` 双许可
2. 本项目会链接 **Live2D Cubism Core**（闭源静态库，C ABI）。Live2D 对 SDK 的使用与再分发有独立的授权条款，需要确认：
   - 本仓库能否包含 Cubism Core 二进制（**倾向：不包含**，由用户自行下载）
   - 项目许可证是否与 Live2D 条款冲突
3. 项目本身不分发游戏资产，这部分无冲突

## 待决事项

- [ ] 确认 Live2D Cubism SDK 的授权条款对本项目形态（开源工具、非商用、不分发 SDK 二进制）的要求
- [ ] 确定许可证，更新 `Cargo.toml` 的 `license` 字段与 `LICENSE` 文件
- [ ] 在 README 中明确：Cubism Core 需用户自行获取并同意 Live2D 条款

## 当前占位

`Cargo.toml` 暂填 `MIT OR Apache-2.0`，**在本 ADR 转为 accepted 前不得对外发布**。
