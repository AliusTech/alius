# Contributing to Alius

感谢你对 Alius 项目感兴趣！

## 开发环境设置

1. 安装 Rust: https://rustup.rs
2. 克隆仓库:
   ```bash
   git clone https://github.com/AliusTech/alius.git
   cd alius
   ```
3. 构建项目:
   ```bash
   cargo build
   ```

## 项目架构

### 核心模块

- `src/main.rs` - 入口点，初始化和路由
- `src/cli.rs` - CLI 命令定义 (使用 clap)
- `src/config.rs` - 配置加载和管理
- `src/error.rs` - 错误类型定义

### 功能模块

- `src/llm/client.rs` - LLM API 客户端封装
- `src/repl/mod.rs` - 交互式 REPL 实现
- `src/ui/welcome.rs` - 欢迎界面渲染

## 添加新功能

### 添加新的斜杠命令

编辑 `src/repl/mod.rs`：

```rust
async fn handle_command(&mut self, input: &str) -> Result<bool> {
    match input.trim() {
        "/your-command" => self.your_command().await?,
        // ...
    }
}

async fn your_command(&mut self) -> Result<()> {
    // 实现逻辑
    Ok(())
}
```

### 添加新的模型

编辑 `src/repl/mod.rs` 中的 `AVAILABLE_MODELS` 数组：

```rust
const AVAILABLE_MODELS: &[&str] = &[
    // 添加新模型
    "new-model-name",
];
```

### 添加新的配置项

1. 编辑 `src/config.rs` 添加字段
2. 更新 `config/default.toml`
3. 更新文档

## 代码风格

- 使用 `cargo fmt` 格式化代码
- 使用 `cargo clippy` 检查代码质量
- 遵循 Rust 命名约定

## 提交规范

提交信息格式：

```
<type>: <description>

[optional body]
```

类型：
- `feat`: 新功能
- `fix`: Bug 修复
- `docs`: 文档更新
- `refactor`: 重构
- `test`: 测试
- `chore`: 构建/工具

## 发布流程

1. 更新 `Cargo.toml` 版本号
2. 合并到 `release` 分支
3. GitHub Actions 自动构建和发布

## 问题反馈

在 GitHub Issues 提交问题或功能请求。