# Rust 迁移进度更新：Anthropic 真流式与接口校验

**日期**: 2026-03-04  
**范围**: `/api/agent/process`、`/api/models` 与 Python 语义契约一致性

## 本轮已完成

1. `/api/agent/process` 在 `stream=true` 下接入真实流式通路（不再仅做整段拆块伪流式）。
2. `active_llm.provider_id=anthropic` 时，新增真正 Anthropic SSE 流解析。
3. 保留降级逻辑：流式初始化失败时，回退到现有 completion 方案，避免前端彻底不可用。
4. 兼容非标准返回：OpenAI/Anthropic 解析器增加“非 SSE JSON”容错（用于 mock/代理场景）。

## 关键代码位置

- `crates/app/src/routes/agent.rs`
  - `agent_process` stream 主分支改造（真实流接入）
  - `start_active_llm_stream` 增加 anthropic 分支
  - `parse_anthropic_stream_line` / `stream_anthropic_completion` 新增
  - `parse_openai_stream_line` 增加非 SSE JSON 容错

## 新增/更新测试

1. `test_agent_process_stream_with_anthropic_active`（新增）
2. `test_agent_process_stream_completed_response_has_output_content`（回归通过）
3. `python_rust_api_contract_test`（语义路由契约回归通过）

## 校验结果

- `cargo test -p copaw routes::agent::tests::test_agent_process_stream_completed_response_has_output_content -- --nocapture` ✅
- `cargo test -p copaw routes::agent::tests::test_agent_process_stream_with_anthropic_active -- --nocapture` ✅
- `cargo test -p copaw --test python_rust_api_contract_test -- --nocapture` ✅

## 待继续验证（联调侧）

1. 前端 Chat 页面在收到 `response.status=completed` 后是否稳定结束 loading。
2. Anthropic 真流式在真实线上模型下的 chunk 粒度与最终拼接一致性。
3. `/api/chats/:id` 历史消息渲染字段与前端卡片标题提取逻辑的最终一致性。
