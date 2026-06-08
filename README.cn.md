# Alius

<p align="center">
  <strong>让软件进入可规划、可执行、可追踪、可自我进化的工程工作区。</strong>
</p>

<p align="center">
  <a href="https://github.com/AliusTech/alius/releases/latest"><img alt="release" src="https://img.shields.io/github/v/release/AliusTech/alius?label=release&style=for-the-badge&color=111111"></a>
  <a href="https://www.npmjs.com/package/@alius-tech/alius"><img alt="npm" src="https://img.shields.io/npm/v/@alius-tech/alius?style=for-the-badge&color=111111"></a>
  <a href="https://github.com/AliusTech/homebrew-tap"><img alt="Homebrew tap" src="https://img.shields.io/badge/homebrew-tap-111111?style=for-the-badge&logo=homebrew&logoColor=white"></a>
</p>

<p align="center">
  <a href="./README.md" aria-label="English">🇺🇸</a>
  &nbsp;&nbsp;
  <a href="./README.cn.md" aria-label="简体中文">🇨🇳</a>
  &nbsp;&nbsp;
  <a href="./README.ja.md" aria-label="日本語">🇯🇵</a>
</p>

Alius 是一个本地优先的 AI Agent Runtime Workspace。它把开发目标转化为可恢复的 Session、可观察的 Run、可审计的 CoreEvent stream，并把配置、记忆、计划和决策沉淀在同一个工程工作区里。

它不是一个套着终端外壳的聊天机器人。Alius 面向的是能参与自身演进的软件：理解目标、生成计划、通过受控运行时执行、记录证据、复盘结果，并把经验带入下一次迭代。

## 为什么是 Alius

| 传统 AI CLI | Alius |
| --- | --- |
| 围绕一段对话组织工作 | 围绕一个真实工程 Workspace 组织工作 |
| 输出文本后交给人解释 | 输出 Plans、Runs、Traces 和可继续执行的上下文 |
| 配置散落在用户机器上 | 项目配置、记忆、工作区文档进入 `.alius/` |
| 把模型选择写成产品假设 | Provider、Base URL、Model、API Key、Soul 都由用户配置 |
| 工具执行像黑盒 | 通过 Protocol、Runtime、Policy、Shell Gate 建立边界 |

## 自进化闭环

Alius 围绕一个核心想法设计：项目应该持续吸收自己的开发过程。

<p align="center">
  <img src="docs/assets/readme/self-evolving-loop.en.svg" alt="Self-Evolving Loop" width="100%">
</p>

每一次迭代都应该留下结构化证据：改了什么、为什么改、使用了什么能力、哪些决策需要审查、哪些经验应该进入记忆。软件的下一次演进，不应该再从空白提示词开始。

## 工程主链

Alius 不让产品入口绕过运行时。CLI、TUI、SDK 和未来 A2A 入口都会通过 Protocol Interface 进入 Core Runtime。

<p align="center">
  <img src="docs/assets/readme/engineering-main-chain.en.svg" alt="Engineering Main Chain" width="100%">
</p>

这条主链让体验层、协议边界、执行引擎、工具、记忆和事件追踪进入同一个工程模型，而不是把能力堆在一组孤立命令上。

## 安装

直接安装最新 release：

```bash
curl -fsSL https://raw.githubusercontent.com/AliusTech/alius/main/scripts/install/install.sh | sh
```

Windows PowerShell：

```powershell
irm https://raw.githubusercontent.com/AliusTech/alius/main/scripts/install/install.ps1 | iex
```

也可以使用包管理器：

```bash
npm install -g @alius-tech/alius

brew tap AliusTech/tap
brew install alius
```

如果本机没有 Homebrew，使用 release 安装脚本或 npm 即可。Alius 不依赖 Homebrew。

验证安装：

```bash
alius --version
```

## 快速开始

初始化当前工程：

```bash
alius init
```

进入 Agent Runtime Workspace：

```bash
alius
```

执行一次非交互请求：

```bash
alius run -p "为这个模块做一次结构化代码审查"
```

## Workspace 体验

Alius 的交互层围绕工作流，而不是围绕“聊天双方”。

- `Chat Mode`：单轮目标澄清、解释、查询和轻量执行。
- `Plan Mode`：多步计划、工具执行、审查和收敛判断。
- `Session`：功能开发、问题修复、代码审查或长期任务的可恢复上下文。
- `Plans`：当前计划节点、状态和后续执行入口。
- `Memory`：项目事实、决策、经验和可复用流程。

常用工作区入口：

```bash
/init
/mode plan
/config
/model
/session new
/memory save <text>
/review
/tools
```

命令不是重点。真正重要的是背后的工作流：从目标到计划，从计划到执行，从执行到证据，从证据到下一次演进。

## 可配置模型运行时

Alius 不把模型列表写死进产品叙事。Provider、Base URL、Model、API Key 和项目 Soul 都是运行时配置。

项目配置保存在当前工作区：

```text
.alius/
├── config/
│   ├── config.toml
│   ├── providers.toml
│   ├── soul.toml
│   ├── tools.toml
│   ├── permissions.toml
│   ├── protocol.toml
│   └── mcp.json
├── memory/
└── workspace/
```

这意味着 Alius 可以接入默认 provider、兼容端点、本地网关或团队代理服务。模型是运行时选择，不是产品边界。

## 为真实工程而设计

Alius 当前聚焦一个清晰边界：在本地工程工作区里，提供可恢复、可审计、可配置、可门禁的 Agent 开发流程。

| 能力 | 设计意图 |
| --- | --- |
| Workspace | 把 AI 工作限定在一个具体工程范围内 |
| Session / Run / Trace | 让开发轮次、执行实例和诊断链路可恢复 |
| Protocol Interface | 让 CLI、TUI、SDK、A2A 使用同一套请求、命令和事件语义 |
| Loop Engine | 用统一执行引擎承载 Chat Mode 和 Plan Mode |
| Shell Gate | 对 shell、process、git 等高风险操作建立策略检查 |
| Memory System | 把项目事实、经验和流程沉淀为长期上下文 |

## 当前成熟度

Alius 已具备 CLI/TUI 主产品、项目初始化、配置向导、Session 基线、Protocol Interface、Core Runtime、Loop Engine 和早期工具/记忆入口。

下一阶段的重点，是让结构化日志、分层记忆、CoreEvent 驱动的 TUI reducer，以及 Shell Gate 策略执行更深入默认路径。
