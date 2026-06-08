# Procedural Memory

程序记忆保存可复用流程、规则、操作模式和项目规范。

## 目标文件

```text
.alius/memory/procedural/procedural.sqlite
```

## 核心表

| 表 | 说明 |
| --- | --- |
| `procedures` | 标准流程 |
| `rules` | 项目规则 |
| `playbooks` | 常见任务处理步骤 |
| `validation_commands` | 验证命令和适用范围 |
| `failure_patterns` | 已知失败模式和修复策略 |

## 写入来源

- `.alius/workspace/ROADMAP.md`。
- `.alius/workspace/docs/modules/` 中的模块验收标准。
- 工程实践中稳定复用的流程。
