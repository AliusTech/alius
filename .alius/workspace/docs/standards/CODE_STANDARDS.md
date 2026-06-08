# Code Standards

更新时间: 2026-06-06 10:45

## 定位

本文定义 Alius 项目的代码提交、分支、检查和 PR 合并标准。所有研发任务都必须遵守该流程；模块设计文档只定义实现内容，本文定义实现如何进入主分支。

## GitFlow 工作流程

Alius 使用以 `master` 为稳定主干的 feature branch 工作流。

发版流程已迁移至 Release Please 自动化，详见 `RELEASE_PROCESS.md`。

标准流程:

```text
同步 master
-> 创建 feature/fix/docs/chore 分支
-> 在分支内完成代码和文档修改
-> 提交信息遵循 [Conventional Commits 格式](./CONVENTIONAL_COMMITS.md)
-> 本地代码检查通过
-> 本地 lint 通过
-> 本地单元测试通过
-> 创建 PR 到 master
-> PR 分支整体检查通过
-> Code Review 通过
-> 合入 master
-> Release Please Bot 自动维护 Release PR
-> 合并 Release PR 触发自动发版
```

## 分支规则

- 禁止直接在 `master` 上开发功能、修复缺陷或提交文档变更。
- 每个研发任务必须先创建独立分支。
- 分支命名建议:
  - `feature/<scope>-<summary>`: 新功能。
  - `fix/<scope>-<summary>`: 缺陷修复。
  - `docs/<scope>-<summary>`: 文档变更。
  - `chore/<scope>-<summary>`: 工程配置、构建、依赖等维护任务。
- 一个分支只处理一个清晰目标，避免把无关改动混在同一个 PR。
- 合并后删除远端临时分支，保持分支列表干净。

## PR 前置检查

创建 PR 前，功能分支必须在本地完成以下检查:

```text
cargo fmt --all -- --check
cargo check -p alius-cli
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -- --test-threads=1
```

检查要求:

- 代码格式检查必须通过。
- 编译/类型检查必须通过。
- lint 必须通过；如果项目后续提供名为 `link` 的脚本、链接检查或文档链接校验，也必须纳入 PR 前置检查并通过。
- 单元测试必须通过。
- 涉及 `cli::formula`、soul、全局状态或共享测试资源时，仓库级测试使用 `cargo test -- --test-threads=1`，避免并发测试状态互相污染。
- 如果项目后续提供统一脚本，例如 `cargo xtask check`、`make check` 或 CI wrapper，可以用脚本替代上述命令，但脚本必须覆盖 format、check、lint、unit test 四类检查。

## PR 合并规则

- 本地检查未全部通过时，不得创建 PR。
- PR 必须指向 `master`。
- PR 分支的 CI 或等价检查必须全部通过。
- PR 必须经过 Review 后才能合入。
- 只有 feature branch 整体检查无问题，才能合到 `master`。
- 合并后如需要删除临时分支，应删除远端 feature 分支，保持分支列表干净。

## 文档同步

- 代码行为变化必须同步更新 `.alius/workspace/docs/` 下对应设计文档。
- 需求或验收标准变化必须同步更新 `.alius/workspace/SPEC.md`。
- 研发阶段或里程碑变化可在 issue、PR 或单独计划文档中维护；`ROADMAP.md` 不作为实现依据。
- 每次文档修改必须追加 `.alius/workspace/HISTORY.md`。

## 验收标准

- 任一 PR 都能追溯到一个独立 feature/fix/docs/chore 分支。
- PR 描述中包含已执行的本地检查命令和结果。
- PR 合入前，format、check、lint、unit test 均通过。
- `master` 始终保持可构建、可测试、可发布的稳定状态。
