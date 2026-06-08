# Memory Cache

本目录保存可重建缓存，例如:

- embedding 临时批处理结果。
- 文档 chunk 缓存。
- 检索 rerank 缓存。
- 压缩摘要中间结果。

缓存损坏时应清理并重建，不应阻断核心执行路径。
