# Conventional Commits 代码规范

> ⚠️ **提交前必读**：每次 `git commit` 前，请确认本文件内容。
> 可运行 `git config commit.template .gitmessage` 让 git 每次提交时自动显示提示。

---

## 提交格式

```
<类型>[可选作用域]: <描述>

[可选正文]

[可选脚注]
```

- **类型**：必须是以下关键字之一，小写
- **作用域**：可选，用括号包裹，如 `(tui)`、`(core)`、`(deps)`
- **描述**：简短描述，不超过 50 字符，小写开头，不加句号
- **正文**：可选，解释 `what` 和 `why`（而非 `how`）
- **脚注**：用于标注 `Breaking Change` 或 `Closes #123` 等

---

## 关键字与版本类型对照表

### 决定版本号的关键字

| 关键字 | 版本变化 | 说明 | CHANGELOG 分组 |
|--------|---------|------|----------------|
| `fix:` | PATCH `0.0.1 → 0.0.2` | Bug 修复 | Fixes |
| `feat:` | MINOR `0.0.2 → 0.1.0` | 新功能 | Features |
| `feat!:` / `fix!:` / 含 `BREAKING CHANGE:` | MAJOR `0.1.0 → 1.0.0` | 破坏性变更 | Breaking Changes |

### 不决定版本号的关键字（计入 CHANGELOG，版本 +PATCH）

| 关键字 | CHANGELOG 分组 |
|--------|----------------|
| `chore:` | Chores |
| `docs:` | Documentation |
| `style:` | Styles（格式化，不影响逻辑） |
| `refactor:` | Code Refactoring |
| `perf:` | Performance Improvements |
| `test:` | Tests |
| `build:` | Build System |
| `ci:` | Continuous Integration |

---

## Breaking Change 写法

### 方式一：在关键字后加 `!`

```bash
feat!: redesign config file format

BREAKING CHANGE: config.toml format updated, see MIGRATION.md
```

### 方式二：在脚注中写 `BREAKING CHANGE:`

```bash
feat: add new API endpoint

BREAKING CHANGE: the old /v1 endpoint is removed, use /v2 instead.
```

> `BREAKING CHANGE:` 后面的文本会完整出现在 CHANGELOG 的 Breaking Changes  section。

---

## 正确示例

```bash
# ✅ 补丁版本
fix: correct model selection error on startup
fix(core): handle 429 rate limit response

# ✅ 小版本
feat: add /history command
feat(tui): support arrow key navigation in model select

# ✅ 大版本（破坏性变更）
feat!: change config file format from JSON to TOML

BREAKING CHANGE: existing config.json will no longer be read.
Migrate by running `alius migrate-config`.

# ✅ 其他类型（PATCH bump）
chore: update release-please configuration
docs: add CONVENTIONAL_COMMITS.md
ci: split workflows into ci and release
refactor(core): extract model selection into separate module
perf: reduce memory usage in chat streaming by 30%
test: add unit tests for model_select module
build: upgrade tokio to 1.40
style: reformat code with rustfmt

# ✅ 带作用域
fix(tui): correct cursor position in model select
feat(npm): add platform-specific package bindings

# ✅ 关闭 Issue
fix: correct startup crash on Windows

Closes #42
```

---

## 错误示例

```bash
# ❌ 缺少冒号——不识别，会被忽略
fix correct error

# ❌ 关键字拼写错误——不识别
fixed: correct error
feature: add new command
Feature: add new command

# ❌ 大写关键字——不识别（必须是小写）
Fix: correct error
FEAT: add feature

# ❌ 描述以大写开头——可以识别，但不推荐
fix: Correct startup error

# ❌ 描述以句号结尾——可以识别，但不推荐
fix: correct startup error.

# ❌ 语义上是 fix，关键字写了 feat——Release Please 按 feat 处理（MINOR bump）
feat: fix typo in README
# 应改为：
fix: fix typo in README

# ❌ 一个 commit 包含多个不相关改动——应拆成多个 commit
feat: add history command and fix model selection error
# 应改为两个 commit：
#   feat: add /history command
#   fix: correct model selection error
```

---

## 与 Release Please 的联动

```
push commit (feat/fix/...) → master
  ↓
release-please.yml 扫描 commit 信息
  ↓
自动更新（或创建）Release PR
  ↓
PR 中包含：CHANGELOG.md 草稿 + 版本号 bump
  ↓
Review 后合并 Release PR
  ↓
Release Please 自动创建 tag（如 v0.0.1）
  ↓
tag 触发 release.yml → 构建 + 发布
```

### 版本 bump 规则（pre-1.0.0 阶段）

| 当前版本 | `fix:` | `feat:` | `feat!:`，`BREAKING CHANGE` |
|---------|--------|---------|---------------------------|
| `0.x.x` | `0.x.x+1` | `0.x+1.0` | `0.x+1.0`（不跳到 1.0.0）|

> `bump-minor-pre-major: false` 配置使 pre-1.0.0 阶段的 `feat` 做 PATCH bump。
> 当前配置为 `false`，所以 `feat:` 在 `0.x.x` 时做 patch bump（`0.0.1 → 0.0.2`）。

---

## 提交前检查清单

- [ ] 关键字小写，后跟冒号 `:` 和空格
- [ ] 描述不超过 50 字符
- [ ] 描述小写开头，无句号
- [ ] 破坏性变更正确标注 `!` 或 `BREAKING CHANGE:`
- [ ] 一个 commit 只做一件事
- [ ] 运行 `git log --oneline -5` 检查格式

---

## 常用 Commit 类型速查

```
fix:     修 Bug       → PATCH
feat:    新功能       → MINOR (pre-1.0: PATCH)
chore:   琐事/配置    → PATCH
docs:    文档         → PATCH
style:   格式化       → PATCH
refactor: 重构        → PATCH
perf:    性能优化     → PATCH
test:    测试         → PATCH
build:   构建系统     → PATCH
ci:      CI 配置      → PATCH
feat!:   破坏性变更   → MAJOR (pre-1.0: MINOR)
```
