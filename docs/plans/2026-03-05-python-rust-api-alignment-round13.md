## Python -> Rust API 对齐进度（Round 13）

日期：2026-03-05

### 本轮目标
- 继续降低“前端页面靠人工点测才发现问题”的风险。
- 增加对关键接口响应形状的回归保护，避免 Rust 端改动引发前端 `TypeError`。

### 本轮已完成
1. 增加 `/config/channels/types` 路由级响应形状测试  
   - 文件：`crates/app/src/routes/config.rs`  
   - 新增测试：`test_router_channel_types_returns_array_shape`  
   - 断言：
     - 状态码 `200`
     - 响应体是 JSON Array（不是 Object）
     - 包含 `console` 渠道

2. 保持既有核心回归测试通过  
   - `/api/models/active` 持久化回归（Round 12）  
   - Python 风格会话文件读取回归（Round 12）  
   - `/api/agent/process` 流式 completed 事件输出与结束信号回归（Round 12）

### 校验结果
- `cargo test -p copaw`：通过（225 passed, 0 failed）
- `cargo test -p copaw --test python_rust_api_contract_test`：通过（1 passed, 0 failed）

### 影响与结论
- 已对 `/sessions` 页面的高风险点之一（`uniqueChannels.map is not a function`）增加后端回归防线：  
  后端若把 `/config/channels/types` 误改成对象结构，将在 Rust 测试阶段直接失败。

### 仍建议联调确认
1. 前端运行时 `BASE_URL` 与 dev proxy 是否一致指向 Rust 服务端。  
2. `/api/models/active` 在你当前环境中的持久化路径（`COPAW_SECRET_DIR/providers.json`）是否存在历史目录冲突。  
3. `/api/chats/{id}` 返回消息后，前端是否按预期渲染（主要是 message content 结构兼容性）。
