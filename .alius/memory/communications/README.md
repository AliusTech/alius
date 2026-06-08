# Communications Memory

本目录保存项目沟通会话记录。

目标结构:

```text
communications/
  sessions/
    <session-id>/
      session.json
      messages.jsonl
```

后续 Core Runtime 重建后，`CoreEvent` trace 可以按 session 或 run 写入本目录，或写入 episodic memory 后在此保留引用。
