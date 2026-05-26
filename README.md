# Alius

LLM Agent CLI Tool - 一个交互式的 LLM 命令行工具。

## 功能特性

- 🖥️ **交互式 REPL 模式** - 支持斜杠命令和实时聊天
- 🔄 **多模型支持** - OpenAI、Anthropic、Gemini 等主流模型
- ⚙️ **灵活配置** - 支持 YAML/TOML/JSON 配置文件
- 🎨 **美观界面** - ASCII Logo 和彩色输出

## 快速开始

### 安装

从 [Releases](https://github.com/AliusTech/alius/releases) 下载对应平台的二进制文件：

```bash
# macOS/Linux
chmod +x alius
mv alius ~/.local/bin/

# 或使用 cargo 安装
cargo install --path .
```

### 配置

设置 API Key 环境变量：

```bash
export OPENAI_API_KEY=your-api-key
# 或其他 provider
export ANTHROPIC_API_KEY=your-api-key
```

### 使用

```bash
# 进入交互模式
alius

# 直接运行任务
alius run -p "解释什么是 Rust"

# 查看配置
alius config show
```

## 交互命令

在 REPL 模式中：

| 命令 | 功能 |
|------|------|
| `/model` | 选择模型 |
| `/config` | 显示配置 |
| `/help` | 显示帮助 |
| `/quit` | 退出 |

## 配置文件

配置位于 `~/.alius/config.toml`：

```toml
[llm]
provider = "openai"
model = "gpt-4o-mini"
api_key_env = "OPENAI_API_KEY"
base_url = "https://api.openai.com/v1"

[agent]
max_retries = 3
timeout_seconds = 60
```

## 项目结构

```
alius_cli/
├── Cargo.toml          # 项目配置和依赖
├── config/
│   └── default.toml    # 默认配置（嵌入二进制）
├── src/
│   ├── main.rs         # 入口点
│   ├── cli.rs          # CLI 命令定义
│   ├── config.rs       # 配置管理
│   ├── error.rs        # 错误类型
│   ├── llm/
│   │   └ mod.rs
│   │   └ client.rs     # LLM 客户端
│   ├── repl/
│   │   └ mod.rs        # 交互式 REPL
│   └── ui/
│       ├── mod.rs
│       └ welcome.rs    # 欢迎界面
└── .github/
    └── workflows/
        └ release.yml   # 自动发布
```

## 支持的模型

- OpenAI: `gpt-4o`, `gpt-4o-mini`, `gpt-4-turbo`, `gpt-3.5-turbo`
- Anthropic: `claude-3-5-sonnet`, `claude-3-opus`, `claude-3-haiku`
- Google: `gemini-1.5-pro`, `gemini-1.5-flash`

## 开发

```bash
# 克隆项目
git clone https://github.com/AliusTech/alius.git
cd alius

# 构建
cargo build

# 运行测试
cargo test

# 发布构建
cargo build --release
```

## License

MIT License - 详见 [LICENSE](LICENSE) 文件