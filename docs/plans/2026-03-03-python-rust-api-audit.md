# CoPaw Python -> Rust 接口迁移排查（第十一轮）

**日期**: 2026-03-03  
**轮次**: Round 11（最新）  
**口径**: 仅统计 Rust 在 `main.rs` 已挂载的路由

## 1. 结论

- 本轮主要是稳定性迭代，整体指标与上一轮持平。
- 绝大多数 API 已完成精确或语义对齐，剩余问题已收敛到极少数边界项。

## 2. 总览统计

| 指标 | 数量 |
|---|---:|
| Python 可见接口总数 | 78 |
| Rust 已挂载接口总数 | 96 |
| 精确匹配（method+path） | 42 |
| 语义匹配（路径参数写法差异） | 35 |
| 未精确匹配 | 36 |
| 未语义匹配 | 1 |

## 3. 分组进度

| 分组 | Python 总数 | 精确匹配 | 语义匹配 | 未匹配 |
|---|---:|---:|---:|---:|
| `/api/agent` | 8 | 4 | 4 | 0 |
| `/api/chats` | 6 | 3 | 3 | 0 |
| `/api/config` | 7 | 5 | 2 | 0 |
| `/api/console` | 1 | 1 | 0 | 0 |
| `/api/cron` | 9 | 2 | 7 | 0 |
| `/api/envs` | 3 | 2 | 1 | 0 |
| `/api/local-models` | 5 | 3 | 2 | 0 |
| `/api/mcp` | 6 | 2 | 4 | 0 |
| `/api/models` | 10 | 4 | 6 | 0 |
| `/api/ollama-models` | 5 | 3 | 2 | 0 |
| `/api/skills` | 11 | 7 | 4 | 0 |
| `/api/version` | 1 | 1 | 0 | 0 |
| `/api/workspace` | 2 | 2 | 0 | 0 |
| `root/static` | 4 | 3 | 0 | 1 |

## 4. 关键评审发现

1. `local-models` 删除模型路径使用通配符 `/*model_id`，与 Python `/{model_id:path}` 仍为语义匹配（建议保留统一转换并补边界测试）。
2. `root/static` 仍由 fallback 覆盖，而非显式 `/{full_path:path}` 路由。

## 5. 明细追踪

- 详见 Tracker：`docs/plans/2026-03-03-api-migration-tracker.md`
