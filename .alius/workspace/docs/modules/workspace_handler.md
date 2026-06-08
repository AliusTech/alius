# Workspace Handler

更新时间: 2026-06-04 22:10

## 模块职责

Workspace Handler 负责 `.alius/workspace/` 的初始化、读写、索引、历史记录和 `.archive/` 已完成版本确认。`ROADMAP.md` 只作为非权威参考，不作为实现依据。

输入:

- 文档创建请求。
- 文档更新请求。
- 模块设计变更。
- assets 引用。
- archive 对比请求。
- archive 确认更新请求。

输出:

- 更新后的 Markdown 文档。
- `HISTORY.md` 追加记录。
- semantic memory 文档索引请求。
- `ArchiveDiffReport`。
- `ArchiveUpdateReport`。

## 接口定义

```text
init_documents(root: Path) -> Result<void>
```

```text
update_document(path: Path, patch: DocumentPatch) -> Result<DocumentUpdateReport>
```

```text
append_history(entry: HistoryEntry) -> Result<void>
```

```text
validate_document_set(root: Path) -> Result<ValidationReport>
```

```text
compare_archive(root: Path) -> Result<ArchiveDiffReport>
```

返回:

- 新增文件。
- 删除文件。
- 修改文件。

```text
confirm_archive(root: Path) -> Result<ArchiveUpdateReport>
```

返回:

- 覆盖到 `.archive/` 的文件数量。
- 删除的旧归档文件数量。
- 归档后校验结果。

## 内部逻辑

```text
update request
-> validate target path under .alius/workspace
-> apply patch
-> validate markdown
-> validate 更新时间 uses YYYY-MM-DD HH:MM when present
-> append HISTORY.md
-> request semantic re-index
```

归档确认逻辑:

```text
confirmation request
-> validate workspace documents
-> compare .alius/workspace with .alius/workspace/.archive
-> wait for user confirmation
-> clear .archive old snapshot
-> copy workspace files to .archive, excluding .archive itself
-> keep relative paths
-> compare again
-> append HISTORY.md
```

## 数据存储

| 路径 | 说明 |
| --- | --- |
| `.alius/workspace/SPEC.md` | 需求源头 |
| `.alius/workspace/ROADMAP.md` | 非权威 Roadmap 说明 |
| `.alius/workspace/HISTORY.md` | 文档修改历史 |
| `.alius/workspace/docs/terms/` | 核心术语 |
| `.alius/workspace/docs/products/` | 产品设计 |
| `.alius/workspace/docs/technology/` | 技术选型 |
| `.alius/workspace/docs/interfaces/` | 分层接口契约 |
| `.alius/workspace/docs/overview/` | 概要设计 |
| `.alius/workspace/docs/modules/` | 模块详细设计 |
| `.alius/workspace/assets/` | 图表和附件 |
| `.alius/workspace/.archive/` | 已完成版本快照，用于与工作版本对比 |

## 异常处理

- 目标路径越界: 拒绝写入。
- HISTORY 追加失败: 文档更新失败并回滚。
- assets 引用缺失: validation warning。
- `.archive/` 缺失: 自动创建。
- 归档覆盖失败: 保留旧 `.archive/` 并返回错误。
- 归档后对比仍有差异: 返回 validation error，不标记确认完成。

## 与其他模块的关系

- 调用 Semantic Memory 建立文档索引。
- 从 Config Manager 读取 documents root。

## 验收标准

- 所有文档修改都有 HISTORY。
- 含 `更新时间:` 的文档必须精确到分钟级别，格式为 `YYYY-MM-DD HH:MM`。
- Roadmap 不被作为实现依据。
- 产品、接口、技术选型和术语目录存在并可索引。
- 每个模块文档包含标准章节。
- `.archive/` 存在。
- 能识别工作版本相对已完成版本的新增、删除、修改。
- 用户确认后能用工作版本完整覆盖 `.archive/`。
