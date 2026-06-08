# Build Targets And Feature Policy

更新时间: 2026-06-04 22:10

## 模块来源

对应架构图分组:

```text
build_group: 编译目标 / Cargo Features
```

## 构建目标

| 构建目标 | 命令 | Feature | 编译内容 | A2A 策略 |
| --- | --- | --- | --- | --- |
| CLI 构建 | `cargo build --release` | `cli-full/default` | 完整 Core Runtime | 可运行时开关启用 |
| 嵌入式第三方库 / SDK 构建 | `cargo build --release --features embedded-sdk` | `embedded-sdk` | FFI + Core Lite | 默认不编译或关闭 |

## Feature Policy

| Feature | 启用内容 | 禁用内容 |
| --- | --- | --- |
| `cli-full` | tools、memory-standard、optional a2a、local embedding | 无架构图强制禁用项 |
| `desktop-planned` | json-rpc、optional a2a | FFI / Core Lite 默认不启用 |
| `embedded-sdk` | ffi、core-lite、remote model/embedding | heavy tools、LanceDB、local embedding、plugin runtime |

## 架构连线

```text
b_policy -> b_cli
b_policy -> b_embedded
b_cli -> p_cli
b_embedded -> p_embedded
b_embedded -> i_ffi
b_embedded -> m_core_lite
```

## 实现规范

- feature 不只是编译开关，也必须影响模块注册、接口暴露、默认能力和测试矩阵。
- `embedded-sdk` 不能把本地工具运行时、LanceDB、本地 embedding 或重型 plugin runtime 编译进产物。
- CLI 构建可以包含完整 Core Runtime，但 A2A 仍应由运行时配置或命令行开关控制。
- Desktop 目前是规划产品，JSON-RPC 和 A2A 只能作为规划路径记录，不得在文档中写成已上线能力。

## 验收标准

- `cli-full/default` 能构建完整 CLI。
- `embedded-sdk` 依赖树能证明 heavy modules 未被编译进去。
- A2A 在 embedded-sdk 下默认关闭或不编译。
- 每个 feature 对应的模块注册和协议入口都有测试或检查项。
