# ADR-0003: 宿主语言 Rust；Cubism Core 运行时加载

- **状态**：accepted
- **日期**：2026-09-25（运行时加载）；早期决策 2026-09-22

## 背景

- 需要一门能在多个平台产出单一可执行文件、便于控制确定性的语言。
- Live2D 模型变形只能由闭源的 Cubism Core 完成，它提供纯 C ABI；Cubism Native Framework 是 C++ 源码。
- Cubism Core 不能随本项目分发。早期做法是构建时通过 `SSE_CUBISM_CORE_DIR` 静态链接，导致没有 SDK 就无法编译、无法跑测试，也无法发布预编译程序。

## 决策

- 宿主语言为 Rust。
- 允许 FFI，但不编译任何 C/C++ 源码：Cubism Core 通过 C ABI 调用，Framework 的相关逻辑用 Rust 重写。
- **Cubism Core 在运行时加载**（`libloading`，即 `dlopen` / `LoadLibrary`），依次查找：
  1. 环境变量 `SSE_CUBISM_CORE`（动态库文件，或其所在目录）；
  2. 环境变量 `SSE_CUBISM_CORE_DIR`（SDK 的 `Core/` 目录，取其中 `dll/<平台>/` 下的动态库）；
  3. `sse` 可执行文件所在目录；
  4. 系统的动态库搜索路径。
- 找不到时，只有需要 Live2D 的操作报错，并列出尝试过的路径。
- `sse-live2d` 是唯一允许 `unsafe` 的 crate。

## 后果

- 正面：没有 SDK 也能编译和测试；可以发布不含 Core 的预编译程序；用户放入自己下载的 Core 即可使用。
- 负面：Core 缺失的错误从编译期推迟到运行时。

## 复审条件

Live2D 提供了可再分发的 Core，或官方提供 Rust 绑定时。
