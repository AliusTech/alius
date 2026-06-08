# Embedded Core Lite

更新时间: 2026-06-04 22:10

## 模块职责

Embedded Core Lite 是嵌入式第三方库 / SDK 的裁剪运行时，只保留必要 Core 能力。

保留能力:

- 配置。
- 模型路由。
- 远程调用。
- 轻量记忆缓存。

禁用能力:

- 本地工具运行时。
- LanceDB。
- 本地 embedding。
- 重型插件。

## 接口定义

```text
ffi_start(request: FfiRequest) -> Result<FfiRunRef>
```

```text
core_lite_run(request: CoreLiteRequest) -> Stream<CoreLiteEvent>
```

```text
core_lite_configure(config: CoreLiteConfig) -> Result<void>
```

异常:

- FFI payload 无法解码。
- 调用了 Core Lite 禁用能力。
- remote model / memory gateway 不可用。

## 内部逻辑

```text
C ABI FFI Adapter
-> decode request
-> validate embedded capability subset
-> load minimal config
-> route remote model call
-> use light memory cache or remote memory gateway
-> return FFI-safe event/result
```

## 数据存储

Core Lite 只允许:

- minimal config。
- light memory cache。
- remote gateway metadata。

不得依赖:

- LanceDB。
- 本地 embedding index。
- plugin runtime store。

## 构建策略

```text
cargo build --release --features embedded-sdk
crate-type = staticlib / cdylib
```

编译内容:

- FFI。
- Core Lite。
- remote model / embedding gateway client。

默认关闭或不编译:

- A2A。
- heavy tools。
- LanceDB。
- local embedding。
- plugin runtime。

## 与其他模块的关系

- 由 C ABI FFI Adapter 进入。
- 使用 Model Router 的 partial 能力。
- 使用 Memory System 的 light cache。
- 使用 Storage Manager 的 config 子集。
- 使用 Security & Policy Manager 的 subset。

## 验收标准

- embedded-sdk 构建不会拉入 heavy modules。
- FFI 输出结构稳定，适合 C ABI。
- Core Lite 调用本地工具时必须被拒绝。
