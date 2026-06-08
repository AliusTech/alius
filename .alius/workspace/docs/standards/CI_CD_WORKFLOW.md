# Alius CI/CD 触发条件详解

## 📊 三大 Workflow

### 1️⃣ CI (.github/workflows/ci.yml)
**触发条件:** `pull_request` targeting `master`

```
创建/更新 PR → master
    ↓
触发 CI
    ↓
┌─────────────────────────────┐
│ • lint (fmt + clippy)       │
│ • test (cargo test)         │
│ • security (cargo audit)    │
└─────────────────────────────┘
    ↓
PR 必须通过所有检查才能合并
```

**特点:**
- 只在 PR 时运行
- 合并到 master 后不再运行
- `permissions: contents: read` (只读，安全)

---

### 2️⃣ Release Please (.github/workflows/release-please.yml)
**触发条件:** `push` to `master` OR `workflow_dispatch`

```
push 到 master
    ↓
触发 release-please
    ↓
扫描 commits (Conventional Commits)
    ↓
┌───────────────────────────────┐
│ 如果有新的 fix/feat commits:  │
│   • 创建/更新 Release PR      │
│   • 更新 CHANGELOG.md         │
│   • 更新 Cargo.toml 版本      │
│   • 更新 manifest.json        │
└───────────────────────────────┘
```

**行为模式:**
- 每次 push 都运行
- 检测到 `fix:`/`feat:` commits → 更新 Release PR
- 只有 `docs:`/`chore:` → 不创建 PR
- 使用 PAT (`ALIUS_RELEASE_TOKEN`) 确保能触发其他 workflow

**关键配置:**
```yaml
token: ${{ secrets.ALIUS_RELEASE_TOKEN }}  # 必须用 PAT!
release-type: rust                         # 自动处理 Cargo.toml
```

---

### 3️⃣ Release (.github/workflows/release.yml)
**触发条件:** `push` with tag `v*`

```
合并 Release PR
    ↓
release-please 用 PAT 创建 tag (v0.1.0)
    ↓
tag push 触发 Release workflow
    ↓
┌──────────────────────────────────┐
│ 1. lint + test                   │
│ 2. 构建 Linux x64                │
│ 3. 构建 macOS x64 + ARM64        │
│ 4. 构建 Windows x64              │
│ 5. 发布到 npm                    │
│ 6. 更新 Homebrew formula         │
└──────────────────────────────────┘
    ↓
创建 GitHub Release (含构建产物)
```

**特点:**
- 只在 tag push 时运行
- PAT 创建的 tag 才能触发 (GITHUB_TOKEN 不行!)
- 构建所有平台的二进制文件
- 自动发布到 npm 和 Homebrew

---

## 🔄 完整流程图

```
┌─────────────────────────────────────────────────────────────┐
│                    开发阶段                                   │
└─────────────────────────────────────────────────────────────┘
                    ↓
          创建 feature branch
                    ↓
          提交代码 (git commit)
                    ↓
          git push origin feature-branch
                    ↓
    ┌───────────────────────────────┐
    │   GitHub: Create PR → master   │
    └───────────────────────────────┘
                    ↓
    ┌───────────────────────────────┐
    │     触发: CI (ci.yml)         │
    │  ✅ lint / test / security     │
    └───────────────────────────────┘
                    ↓
    ┌───────────────────────────────┐
    │   Review PR → 通过后合并      │
    │   git push origin master       │
    └───────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────────────────┐
│                    准备发布阶段                               │
└─────────────────────────────────────────────────────────────┘
                    ↓
    ┌───────────────────────────────┐
    │ 触发: release-please.yml      │
    │ (每次 push to master 都运行)  │
    └───────────────────────────────┘
                    ↓
          扫描 Conventional Commits
                    ↓
    ┌───────────────────────────────┐
    │  如果有 fix/feat commits:     │
    │  创建/更新 Release PR          │
    │  标题: chore(main): release   │
    │       alius 0.1.0             │
    └───────────────────────────────┘
                    ↓
    ┌───────────────────────────────┐
    │  Review Release PR            │
    │  (检查 CHANGELOG.md)          │
    └───────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────────────────┐
│                    正式发布阶段                               │
└─────────────────────────────────────────────────────────────┘
                    ↓
          合并 Release PR
                    ↓
    ┌───────────────────────────────┐
    │ release-please (用 PAT):      │
    │  1. 更新 Cargo.toml 版本      │
    │  2. 提交 Cargo.lock           │
    │  3. 更新 .release-please-     │
    │     manifest.json             │
    │  4. 创建 tag v0.1.0           │
    │  5. 创建 GitHub Release       │
    └───────────────────────────────┘
                    ↓
          tag push (v0.1.0)
                    ↓
    ┌───────────────────────────────┐
    │ 触发: release.yml              │
    │ (只有 tag push 才触发!)       │
    └───────────────────────────────┘
                    ↓
    ┌───────────────────────────────┐
    │ • 再次 lint + test            │
    │ • 构建 Linux x64              │
    │ • 构建 macOS x64 + ARM64      │
    │ • 构建 Windows x64            │
    │ • 发布 npm packages           │
    │ • 更新 Homebrew               │
    └───────────────────────────────┘
                    ↓
          ✅ 发布完成!
```

---

## ⚠️ 关键注意事项

### 1. 为什么必须用 PAT？

GitHub 的安全限制：`GITHUB_TOKEN` 创建的 tag **不会触发**其他 workflow。

| Token | 触发 workflow? | 用途 |
|-------|---------------|------|
| `GITHUB_TOKEN` | ❌ | GitHub 自动提供，但不能触发其他 workflow |
| `ALIUS_RELEASE_TOKEN` (PAT) | ✅ | 必须用 PAT 才能让 tag 触发 release.yml |

### 2. 什么时候运行什么？

| 操作 | 触发的 workflow |
|------|----------------|
| 创建/更新 PR | CI |
| push 到 master (普通 commit) | release-please |
| push 到 master (merge Release PR) | release-please → 创建 tag → release |
| 手动触发 | release-please |

### 3. 版本号如何确定？

由 release-please 根据 commits 自动决定：

| Commit 类型 | 版本变化 | 示例 |
|-------------|---------|------|
| `fix:` | patch | 0.1.0 → 0.1.1 |
| `feat:` | minor | 0.1.0 → 0.2.0 |
| `feat!/fix!/BREAKING CHANGE:` | major | 0.1.0 → 1.0.0 |
| `docs:/chore:/test:` | 不变 | 不触发发版 |

配置了 `bump-minor-pre-major: false`，所以在 1.0.0 之前，`feat:` 也只 bump patch。

---

## 🧪 测试建议

### 测试 CI
```bash
git checkout -b test/ci-trigger
echo "// test" >> src/test.rs
git commit -m "test: ci trigger test"
git push origin test/ci-trigger
# 创建 PR，观察 CI 是否运行
```

### 测试 release-please
```bash
git checkout master
echo "// test" >> src/lib.rs
git commit -m "feat: test release-please trigger"
git push origin master
# 观察 release-please 是否创建 Release PR
```

### 测试完整发布流程
```bash
# 先提交一个 feat
git commit -m "feat: add test feature"
git push origin master

# 等待 Release PR 创建
# 合并 Release PR

# 观察 tag 是否创建
git tag

# 观察 release.yml 是否运行
```
