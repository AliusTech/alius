# Release Process (Release Please)

更新时间: 2026-06-06 10:45

## 文档定位

本文定义 Alius 项目的标准发版流程，基于 [Release Please](https://github.com/googleapis/release-please) 实现自动化版本管理与发版。所有发版操作必须遵守本文流程。

旧流程（手动改版本号 → 推 `release/*` 分支）已废弃。

---

## 核心原理

Release Please 通过扫描 `master` 分支上的 **Conventional Commits** 历史，自动维护一个 Release PR（标题类似 `chore(main): release alius 0.6.16`）。合并该 PR 后，Bot 自动打 tag → 创建 GitHub Release → 触发 `release.yml` 构建发版。

```
正常提交（feat/fix 格式）
      ↓
Release Please Bot 自动识别有意义 commit
      ↓
自动创建/更新 Release PR（含 CHANGELOG + 版本号）
      ↓
Review 并合并 Release PR
      ↓
Bot 自动打 tag → 创建 GitHub Release
      ↓
触发 release.yml → 构建 → 发包 npm → 更新 homebrew
```

---

## Commit 格式规范（Conventional Commits）

每次提交必须遵循以下格式，否则 Release Please 不会将其计入版本变更：

```
<type>(<scope>): <subject>
```

### Type 与版本号关系

| Type | 版本变化 | 说明 |
| --- | --- | --- |
| `fix` | Patch (`0.6.15` → `0.6.16`) | 缺陷修复 |
| `feat` | Minor (`0.6.15` → `0.7.0`) | 新功能 |
| `feat!` / `fix!` / `refactor!` | Major (`0.6.15` → `1.0.0`) | 破坏性变更，需在 commit 中添加 `BREAKING CHANGE:` 说明 |
| `docs` / `chore` / `test` / `refactor`（无 `!`） | 不触发发版 | 仅文档/工程/测试变更 |

### 示例

```bash
# Patch 发版
git commit -m "fix(cli): correct model list caching on 429 error"

# Minor 发版
git commit -m "feat(tui): add streaming response preview in conversation panel"

# Major 发版（破坏性变更）
git commit -m "feat!: migrate to release-please for automated releases

BREAKING CHANGE: manual release/* branch workflow is removed"

# 不触发发版
git commit -m "docs: update release process documentation"
git commit -m "chore(deps): bump async-openai to 0.24"
```

---

## 发版步骤

### 日常发版（标准流程）

1. **正常提交代码**到 `master`（遵循 Conventional Commits 格式）
2. **Release Please Bot 自动**在仓库创建/更新 Release PR
3. **Review Release PR**：检查 `CHANGELOG.md` 变更描述是否准确
4. **合并 Release PR**
5. **自动完成**：Bot 打 tag → 创建 GitHub Release → 触发 CI 构建发版

### 手动指定版本号

如需强制指定版本号（非标准语义版本），在合并 Release PR 前修改 PR 中的 `.release-please-manifest.json`：

```json
{
  ".": "0.6.16"
}
```

或在 `release-please-config.json` 中配置 `bump-minor-pre-major: true` 控制 Major 版本行为。

---

## 配置文件说明

| 文件 | 作用 |
| --- | --- |
| `.github/workflows/release-please.yml` | CI 工作流：监听 `master` push，驱动 Release Please |
| `release-please-config.json` | Release Please 策略配置（`release-type: rust`） |
| `.release-please-manifest.json` | 当前版本追踪文件（被 Bot 自动更新） |
| `CHANGELOG.md` | 发版时自动更新，记录每个版本的变更内容 |
| `Cargo.toml` | Rust workspace 版本文件（自动更新） |
| `Cargo.lock` | Rust 依赖锁文件（自动提交） |

**禁止手动修改** `.release-please-manifest.json`、`CHANGELOG.md` 和 `Cargo.toml` 版本号，这些文件由 Bot 维护。

### release-please-config.json 配置

```json
{
  "packages": [{
    "package-name": "alius",
    "release-type": "rust",           // Rust 项目专用类型
    "path": ".",
    "bump-minor-pre-major": false,    // pre-1.0 时 feat 也 bump patch
    "rust-package-lock-file": "Cargo.lock"  // 自动提交 lockfile
  }]
}
```

---

## PAT 配置（关键）

### 为什么需要 PAT？

GitHub 默认 `GITHUB_TOKEN` 推送的 tag **不会触发其他 workflow**（安全限制）。必须用 PAT 才能让 Release Please 打的 tag 触发 `release.yml` 构建发版。

### 配置步骤

1. 创建 PAT：https://github.com/settings/tokens/new
   - **Token name**：`alius-release-please`
   - **Expiration**：`90 days`
   - **Scopes**：勾选 `repo`（全选子项）
2. 复制生成的 token
3. 在仓库 Settings → Secrets and variables → Actions 中添加 secret：
   - **Name**：`ALIUS_RELEASE_TOKEN`
   - **Secret**：粘贴 token

### 验证 PAT 生效

检查 `.github/workflows/release-please.yml` 中 `token:` 字段值为 `${{ secrets.ALIUS_RELEASE_TOKEN }}`。

---

## 故障排查

### Release PR 未自动创建

- 检查 `master` 上是否有新的 Conventional Commits（非 `docs`/`chore`/`test`）
- 检查 `.github/workflows/release-please.yml` 是否启用
- 检查 `ALIUS_RELEASE_TOKEN` secret 是否配置正确

### Tag 推送后 `release.yml` 未触发

- 确认使用的是 PAT（`ALIUS_RELEASE_TOKEN`）而非 `GITHUB_TOKEN`
- 检查 tag 格式是否为 `v*`（`release.yml` 触发条件：`tags: ['v*']`）

### CHANGELOG.md 变更描述不准确

在 Release PR 合并前，直接编辑 PR 中 `CHANGELOG.md` 的对应段落，Release Please 会保留手动编辑的内容。

---

## 验收标准

- 所有发版操作均通过 Release Please 完成，无手动改版本号
- 所有有意义 commit 遵循 Conventional Commits 格式
- `ALIUS_RELEASE_TOKEN` PAT 已配置且未过期
- `release.yml` 能被 tag 正确触发
- `CHANGELOG.md` 准确反映每个版本的变更内容
