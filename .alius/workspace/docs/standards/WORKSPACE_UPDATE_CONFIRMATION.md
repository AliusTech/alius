# Workspace Update Confirmation

更新时间: 2026-06-04 22:10

## 定位

本文定义 `.alius/workspace/` 文档更新的确认、对比和归档流程。目标是区分“工作版本”和“已完成版本”，让文件新增、删除、修改都能被直观核对。

## 目录结构

```text
.alius/workspace/
├── .archive/        # 已完成版本存档
├── SPEC.md          # 工作版本
├── ROADMAP.md       # 工作版本，非权威参考
├── HISTORY.md       # 工作版本
├── docs/            # 工作版本
└── assets/          # 工作版本
```

## 版本定义

| 位置 | 版本角色 | 说明 |
| --- | --- | --- |
| `.alius/workspace/` | 工作版本 | 用户日常创建、编辑、删除文档的主目录 |
| `.alius/workspace/.archive/` | 已完成版本 | 保存上一次确认定稿后的完整 workspace 文档快照 |

## 归档结构约定

`.archive/` 不创建版本号子目录，例如不使用 `.archive/v1/`、`.archive/2026-06-04/`。

工程修正:

- 当前 workspace 已包含 `docs/`、`assets/` 等子目录。
- 如果 `.archive/` 完全扁平化且不保留子目录，目录 diff 工具无法直接判断嵌套文件的修改和删除。
- 因此本项目采用“无版本嵌套、保留相对路径”的归档方式: `.archive/` 内部直接保存一份完整 workspace 快照，但相对路径与工作区保持一致。

示例:

```text
.alius/workspace/docs/overview/ARCH.md
-> .alius/workspace/.archive/docs/overview/ARCH.md

.alius/workspace/SPEC.md
-> .alius/workspace/.archive/SPEC.md
```

## 对比逻辑

查看工作版本与已完成版本差异时，对比:

```text
.alius/workspace/
.alius/workspace/.archive/
```

对比时必须排除:

```text
.alius/workspace/.archive/
.gitkeep
```

识别规则:

- 新增文件: 工作版本存在，但 `.archive/` 不存在。
- 删除文件: `.archive/` 存在，但工作版本不存在。
- 修改文件: 工作版本和 `.archive/` 中同相对路径文件内容不同。

推荐工具:

- `diff -ru --exclude .archive --exclude .gitkeep .alius/workspace .alius/workspace/.archive`
- Beyond Compare。
- Kaleidoscope。
- IDE directory compare。

## 更新确认流程

```text
编辑 workspace 文档
-> 运行文档检查
-> 对比 workspace 与 .archive
-> 用户确认工作版本定稿
-> 用工作版本完整覆盖 .archive
-> 追加 HISTORY.md
```

## 归档覆盖规则

当工作版本确认定稿后:

1. 清空 `.alius/workspace/.archive/` 中旧快照。
2. 从 `.alius/workspace/` 复制所有应归档文件到 `.archive/`。
3. 复制时排除 `.archive/` 本身。
4. 保留原相对路径。
5. 归档后再次执行目录对比，确认无差异。

## 归档范围

默认归档:

- `SPEC.md`
- `ROADMAP.md`，非权威参考
- `HISTORY.md`
- `docs/`
- `assets/`

默认不归档:

- `.archive/` 本身。
- `.archive/.gitkeep` 占位文件。
- 临时文件。
- 编辑器 swap 文件。
- 系统文件如 `.DS_Store`。

## Workspace Handler 要求

Workspace Handler 后续应提供以下能力:

```text
compare_archive(root: Path) -> Result<ArchiveDiffReport>
```

```text
confirm_archive(root: Path) -> Result<ArchiveUpdateReport>
```

```text
validate_archive(root: Path) -> Result<ArchiveValidationReport>
```

## 验收标准

- `.alius/workspace/.archive/` 存在。
- 对比工具能识别新增、删除、修改。
- 确认归档后，工作版本与 `.archive/` 快照无差异。
- 每次确认归档都追加 `HISTORY.md`。
