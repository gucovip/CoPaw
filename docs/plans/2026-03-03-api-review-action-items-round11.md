# CoPaw API 审查整改意见（Round 11）

**日期**: 2026-03-03  
**来源**: 基于 Round 10 审查结果，仅保留需要修改的接口项  
**筛选标准**:
- 未语义匹配（必须整改）
- 语义匹配但可能影响前端调用（需要整改）

## 1. 必改清单

| 优先级 | 类型 | Python 契约 | Rust 现状 | 风险说明 | 修改建议 |
|---|---|---|---|---|---|
| P0 | 未语义匹配 | `GET /{full_path:path}` | 仅使用全局 fallback 返回 SPA | 深链刷新一般可用，但不是显式同路径路由，契约审计难闭环 | 增加显式 `GET /*full_path -> spa_fallback_handler`；保留 fallback 兜底，并保证 `/api/*` 路由优先匹配 |
| P1 | 语义匹配（高风险） | `DELETE /api/local-models/{model_id:path}` | `DELETE /api/local-models/*model_id` | `*` 通配符解析边界（前导 `/`、编码）可能导致前端删除失败 | 在 handler 入口统一规范化 `model_id`（去前导 `/`、统一 decode 行为），并补齐路径边界测试 |
| P1 | 语义匹配（高风险） | `DELETE /api/ollama-models/{name:path}` | `DELETE /api/ollama-models/*name` | 与上同，模型名含层级分隔符或编码时可能行为不一致 | 同步规范化 `name` 参数，并补齐边界测试 |
| P1 | 语义匹配（中风险） | `GET /api/skills/{skill_name}/files/{source}/{file_path:path}` | `GET /api/skills/:skill_name/files/:source/*file_path` | `file_path` 多级路径解析/编码差异会影响文件读取 | 统一规范化 `file_path`（去前导 `/`），补齐多级目录与编码路径测试 |

## 2. 涉及代码位置

- SPA fallback：
  - `crates/app/src/main.rs`
- Local models：
  - `crates/app/src/routes/local_models.rs`
- Ollama models：
  - `crates/app/src/routes/ollama_models.rs`
- Skills files：
  - `crates/app/src/routes/skills.rs`

## 3. 验收标准

1. 上述 4 项接口在 tracker 中不再属于“未语义匹配/高风险语义匹配”。
2. 新增集成测试覆盖以下用例：
   1. 路径参数包含 `/` 的情况。
   2. 路径参数包含 `%2F` 的情况。
   3. 普通单层参数路径。
3. 对前端调用无感（无需改前端调用路径与参数拼接逻辑）。

## 4. 说明

- 本文档只记录“需要修改”的接口，不包含低风险语义差异（如普通 `{id}` vs `:id`）。
