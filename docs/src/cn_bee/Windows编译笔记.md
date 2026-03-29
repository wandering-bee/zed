# Windows 编译笔记

## 环境要求

在 Zed 官方文档基础上，Windows 编译还需要注意以下几点：

### CMake

wasmtime-c-api-impl 需要 CMake，官方文档有提但容易遗漏。

```
winget install Kitware.CMake
```

安装后需要重启终端让 PATH 生效。

### Windows SDK 版本

webrtc-sys 编译 flag 定义了 `NTDDI_VERSION=NTDDI_WIN11_GE`，需要较新的 Windows SDK。

- 最低要求：**Windows 11 SDK (10.0.26100.0)**
- 通过 Visual Studio Installer → 单个组件安装

### MSVC 工具链与 Spectre 库版本匹配

webrtc-sys 的预编译库使用了较新版本 MSVC STL 的内部符号（如 `__std_find_first_of_trivial_pos_1`）。如果链接阶段报 LNK2019/LNK2001 找不到该符号：

1. 确保 MSVC build tools 升级到最新版本（如 v14.44+）
2. **同时安装对应版本的 Spectre-mitigated libs**（如 v14.44-17.14）
3. 执行 `cargo clean` 后重新编译

Spectre libs 版本必须和 MSVC build tools 版本一致，否则链接器会加载旧版 STL。

## 编译命令

```bash
cargo run --release
```

首次全量编译耗时较长（20-40 分钟），后续增量编译会快很多。
