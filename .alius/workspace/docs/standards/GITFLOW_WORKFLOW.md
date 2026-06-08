# GitFlow Workflow

更新时间: 2026-06-06 09:15

## 定位

本文定义 Alius 项目推行 GitFlow 的分支、PR、发布和迁移规则。它补充 `CODE_STANDARDS.md`，用于把当前已经进行一段时间的并行开发收敛到稳定、可审计、可发布的工程流程。

Alius 采用 **简化 GitFlow**:

- `master` 只代表稳定可发布状态。
- `develop` 代表下一版本集成状态。
- 日常开发使用短生命周期工作分支。
- 发布使用 `release/*` 分支冻结和验证。
- 线上紧急修复使用 `hotfix/*` 分支。

## 推行目标

| 目标 | 说明 |
| --- | --- |
| 稳定主分支 | `master` 始终可构建、可测试、可发布 |
| 集成可控 | 所有新功能先进入 `develop`，避免未验证改动直接冲击发布线 |
| 发布可追踪 | 每个版本有 `release/*` 分支、PR、检查记录和 tag |
| 修复可回流 | `hotfix/*` 合入 `master` 后必须回流 `develop` |
| 文档同步 | 代码、配置和流程变化同步 `.alius/workspace/docs/` 与 `HISTORY.md` |

## 分支角色

| 分支 | 来源 | 合入目标 | 用途 | 规则 |
| --- | --- | --- | --- | --- |
| `master` | release/hotfix PR | 无 | 稳定发布主干 | 禁止直接提交；只接受 release/hotfix 合并；保存已发布代码状态 |
| `develop` | `master` 初始化 | release 分支 | 下一版本集成线 | 禁止直接提交；只接受 feature/fix/docs/chore PR |
| `feature/<scope>-<summary>` | `develop` | `develop` | 新功能 | 一个分支一个目标；完成后删除远端分支 |
| `fix/<scope>-<summary>` | `develop` | `develop` | 非线上紧急缺陷 | 用于下一版本修复，不直接进 `master` |
| `docs/<scope>-<summary>` | `develop` | `develop` | 文档和规范 | 影响发布说明时可在 release 分支补充 |
| `chore/<scope>-<summary>` | `develop` | `develop` | 构建、依赖、配置维护 | 不夹带功能行为变化 |
| `release/<release-id>` | `develop` | `master` 和 `develop` | 版本冻结、验证、发版准备 | 只允许 release blocker、版本号、发布文档和低风险修复；只有 `release/*` 会触发发布 CI |
| `hotfix/<version-or-summary>` | `master` | `master` 和 `develop` | 已发布版本紧急修复 | 必须最小改动；合入 `master` 后回流；如需发布补丁，另建 `release/*` |

不再新增 `future/*` 作为长期工作分支。已有 `future/*` 分支按本文“迁移计划”处理。

## 日常开发流程

```text
git fetch origin
git switch develop
git pull --ff-only
git switch -c feature/<scope>-<summary>

# 完成修改
# 运行本地检查

git push -u origin feature/<scope>-<summary>
# 创建 PR: feature/* -> develop
```

PR 合入 `develop` 前必须满足:

- 分支目标单一，PR 描述清楚说明 scope、风险和验证结果。
- 需要同步文档的代码变化已经更新 `.alius/workspace/docs/`。
- 涉及需求、验收或工程流程变化时已更新 `SPEC.md`。
- 文档变化已追加 `HISTORY.md`。
- 本地检查和 CI 通过。
- Code Review 通过。

## 发布流程

发布分支从 `develop` 创建:

```text
git fetch origin
git switch develop
git pull --ff-only
git switch -c release/0.6.16
```

预发布版本也使用同一规则:

```text
git switch -c release/0.7.0-beta.1
```

release 分支命名规则:

- `release/` 后缀作为 GitHub Release tag 和 release name，例如 `release/0.7.0-beta.1` 会创建 tag/release `0.7.0-beta.1`。
- npm、Cargo 构建环境和 Homebrew formula 使用 SemVer 包版本；如果分支后缀带可选前导 `v`，例如 `release/v0.7.0-beta.1`，包版本会归一化为 `0.7.0-beta.1`。
- 分支后缀必须是有效 SemVer，允许预发布和 build metadata，例如 `0.7.0-beta.1`、`0.7.0+build.3`。

release 分支只处理:

- 版本号和包元数据。
- README、安装脚本、发布说明。
- release blocker 修复。
- 文档中的已实现状态和验收状态校准。

发布顺序:

```text
push release/0.6.16
Release workflow creates GitHub Release 0.6.16
Release workflow publishes npm packages and updates Homebrew
release/0.6.16 -> master
release/0.6.16 -> develop
delete release/0.6.16
```

要求:

- `release/*` 合入 `master` 前必须跑完整检查。
- 只有 `release/*` 分支 push 或手动选择 `release/*` 分支运行 workflow 才允许触发 release、npm publish 和 Homebrew 更新。
- GitHub Release tag/name 来自 `release/` 后缀，由 release workflow 创建。
- `master` 合并后必须回流 `develop`，避免版本号、文档或修复丢失。

## Hotfix 流程

线上或已发布版本紧急问题从 `master` 创建 hotfix:

```text
git fetch origin
git switch master
git pull --ff-only
git switch -c hotfix/0.6.16-critical-fix

# 最小修复和验证

git push -u origin hotfix/0.6.16-critical-fix
# 创建 PR: hotfix/* -> master
```

合并后:

```text
hotfix/0.6.16-critical-fix -> master
hotfix/0.6.16-critical-fix -> develop
```

如果当前存在打开的 `release/*` 分支，hotfix 也必须同步进入该 release 分支。

如果 hotfix 需要立即发布补丁版本，必须从修复后的 `master` 创建 release 分支触发发布:

```text
git switch master
git pull --ff-only
git switch -c release/0.6.17
git push -u origin release/0.6.17
```

`hotfix/*` 本身不触发 release、npm publish 或 Homebrew 更新。

## PR 目标

| 分支类型 | PR 目标 |
| --- | --- |
| `feature/*` | `develop` |
| `fix/*` | `develop` |
| `docs/*` | `develop` |
| `chore/*` | `develop` |
| `release/*` | `master`，合并后再回流 `develop` |
| `hotfix/*` | `master`，合并后再回流 `develop` 和当前 release |

禁止把 feature/fix/docs/chore 直接 PR 到 `master`，除非这是 release 分支上的收尾改动。

## 检查门槛

默认 PR 前置检查:

```text
cargo fmt --all -- --check
cargo check -p alius-cli
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -- --test-threads=1
```

release/hotfix 分支必须执行完整 workspace 检查。涉及共享测试资源、soul、formula、全局状态或 session 存储时，仓库级测试使用:

```text
cargo test -- --test-threads=1
```

如果后续引入统一检查脚本，例如 `cargo xtask check`、`make check` 或 CI wrapper，该脚本必须覆盖 format、check、lint、unit test 四类检查，才能替代上述命令。

## 保护规则

建议在远端仓库启用:

- `master` 和 `develop` 禁止直接 push。
- `master` 和 `develop` 禁止 force push。
- 合并前必须通过 CI。
- 合并前至少一次 Review。
- 发布 CI 只允许 `release/<semver>` 分支触发，例如 `release/0.6.16` 或 `release/0.7.0-beta.1`；可选前导 `v` 会在包版本中去掉。
- GitHub Release tag/name 使用 `release/` 后缀；包版本允许去掉可选前导 `v`。
- 已合并的工作分支及时删除。

## 现有分支迁移计划

当前项目已经存在 `future/*` 等开发分支。迁移 GitFlow 时不要一次性重命名所有分支，避免打断正在进行的工作。

建议分三步:

### 第一步: 建立 develop

从当前稳定 `master` 创建 `develop`:

```text
git fetch origin
git switch master
git pull --ff-only
git switch -c develop
git push -u origin develop
```

如果远端已经存在 `develop`，则只同步并保护它:

```text
git switch develop
git pull --ff-only
```

### 第二步: 迁移活动分支

盘点现有活动分支:

```text
git branch -r --list 'origin/future/*'
git branch -r --list 'origin/feature/*'
git branch -r --list 'origin/fix/*'
git branch -r --list 'origin/docs/*'
```

处理规则:

- 已接近完成的 `future/*`: 继续完成，但 PR 目标改为 `develop`。
- 长期未完成的 `future/*`: 基于 `develop` 新建 `feature/*` 或 `fix/*`，只迁移仍有价值的提交。
- 发布准备类 `future/release-*`: 改用 `release/<version>`。
- 文档类 `future/docs-*`: 改用 `docs/<scope>-<summary>`。

### 第三步: 冻结旧命名

完成一轮 release 后:

- 不再创建新的 `future/*` 分支。
- 删除已经合并的旧 `future/*`。
- 将仓库贡献说明、CI 目标分支和 PR 模板统一到本文规范。

## Alius 特别规则

- `.alius/workspace/` 是项目设计和规范事实源，工程流程变化必须更新这里。
- `ROADMAP.md` 只作为阶段参考，不作为实现和合并依据。
- 每次 workspace 文档修改必须追加 `HISTORY.md`。
- 代码行为变化必须同步产品、接口或模块文档。
- release 分支需要核对 `SPEC.md`、`UNIMPLEMENTED.md` 和 README 中的实现状态，避免发布说明夸大未接入主路径的能力。

## 验收标准

- `master` 只包含发布和 hotfix 合并。
- 日常开发 PR 默认进入 `develop`。
- 每个版本都有 `release/<version>` 记录；hotfix 如需发布补丁版本，也必须创建对应 `release/<patch-version>`。
- 每个发布 tag 都能追溯到通过检查和 Review 的 PR。
- hotfix 合入 `master` 后能在 `develop` 找到对应回流提交。
- 旧 `future/*` 分支逐步停止新增并被 feature/fix/docs/chore/release/hotfix 命名替代。
