# 2026-03-05 Python -> Rust 接口对齐审计（Round 12）

## 范围
本轮聚焦前端直接受影响的 4 组接口：

1. `GET/PUT /api/models/active`
2. `GET /api/chats` + `GET /api/chats/{chat_id}`
3. `POST /api/agent/process`（`stream=true`）
4. `/api/config/channels/types`（Sessions 过滤栏依赖）

## 对齐结论（本轮）
| 接口 | Python 行为基线 | Rust 当前行为 | 结论 |
|---|---|---|---|
| `/api/models/active` | 读写 `active_llm` 槽位 | 读写一致，类型与字段一致（`active_llm.provider_id/model`） | ✅ 对齐 |
| `/api/chats/{chat_id}` | 从 `sessions/{safe_uid}_{safe_sid}.json` 读取并转换 memory | 已支持同路径规则、同 memory 结构转换 | ✅ 对齐 |
| `/api/agent/process` 流式 | 事件流按 `response/message/content` 递进，最终 `status=completed/failed` | 已输出 `response`+`message`+`content`，并有 completed/fail 终态 | ✅ 语义对齐 |
| `/api/config/channels/types` | 返回字符串数组 | 返回 `string[]` | ✅ 对齐 |

## 本轮新增回归测试

### 1) models active 回环持久化
- 文件：`crates/app/src/routes/models.rs`
- 用例：`test_set_active_model_roundtrip_persists`
- 目的：防止出现“已配置但 `active_llm` 读取为空”的回归。

### 2) chats 历史读取（Python 风格 session 文件）
- 文件：`crates/app/src/routes/chats.rs`
- 用例：`test_load_session_messages_from_python_style_session_file`
- 目的：防止 `/api/chats/{id}` 返回 `{"messages":[]}` 的回归。

### 3) agent 流式 completed 事件完整性
- 文件：`crates/app/src/routes/agent.rs`
- 用例：`test_agent_process_stream_completed_response_has_output_content`
- 校验点：
  - `status=completed` 的 response 事件 `output` 不能为空
  - completed 事件包含 `completed_at`
  - completed 事件包含 `sequence_number`
- 目的：降低“前端已收到 completed 但未结束渲染”的兼容风险。

## 验证结果
执行：

```bash
cargo test -p copaw
cargo test -p copaw --test python_rust_api_contract_test
```

结果：
- `copaw` 单测：`224 passed, 0 failed`
- Python/Rust 路由语义契约测试：`1 passed, 0 failed`

## 现阶段剩余风险（非阻断）
1. `@agentscope-ai/chat` 对流式完成事件较敏感；若上游模型流异常中断，前端可能表现为“响应未完成”。
2. 本轮已覆盖后端契约层，但仍建议做 1 轮浏览器端联调回放：
   - `/chat` 连续发送 3 轮消息
   - `/sessions` 打开历史会话
   - `/cron-jobs` 列表加载

## 下一步建议
1. 抓取一次真实浏览器网络流（`/api/agent/process`）并落盘，补 1 个“真实流式 transcript 回放测试”。
2. 将上述 4 组接口加入 CI 的 smoke contract（请求 + schema 断言）。
