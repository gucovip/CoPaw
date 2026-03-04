# CoPaw Rust 重构设计文档

**日期**: 2026-03-03
**作者**: Claude
**状态**: 设计阶段

## 1. 概述

将 CoPaw 从 Python 重构为 Rust，实现以下目标：
- **性能提升**: 更低的内存占用和更快的执行速度
- **资源优化**: 减少 CPU 和内存消耗
- **部署简化**: 单一可执行文件，无需 Python 运行时
- **生态优势**: 利用 Rust 生态系统

## 2. 架构设计

### 2.1 目录结构

```
copaw-rust/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── core/                     # 核心抽象和 trait
│   ├── app/                      # HTTP 服务 (Axum)
│   ├── runner/                   # Agent 运行时
│   ├── channels/                 # 消息渠道
│   ├── agents/                   # Agent 实现
│   ├── providers/                # LLM 提供商
│   ├── skills/                   # Skills 系统
│   ├── config/                   # 配置管理
│   ├── memory/                   # 内存管理
│   ├── tools/                    # 内置工具
│   ├── crons/                    # 定时任务
│   ├── mcp/                      # MCP 客户端
│   └── utils/                    # 工具函数
├── console/                      # 保持现有 React 前端
└── scripts/                      # 构建脚本
```

### 2.2 模块依赖图

```
┌─────────────────────────────────────────────────────────────┐
│                        app (HTTP)                           │
│  Axum Server + API Routes + Static File Serving             │
└──────────────────────┬──────────────────────────────────────┘
                       │
       ┌───────────────┼───────────────┐
       ▼               ▼               ▼
┌─────────────┐ ┌──────────────┐ ┌──────────────┐
│  channels   │ │   runner     │ │   crons      │
│  (Feishu...) │ │ (Agent exec) │ │ (Scheduler)  │
└──────┬──────┘ └──────┬───────┘ └──────────────┘
       │                │
       └────────┬───────┘
                ▼
         ┌──────────────┐
         │    agents    │
         │ (ReActAgent) │
         └──────┬───────┘
                │
       ┌────────┼────────┐
       ▼        ▼        ▼
┌──────────┐┌──────┐┌─────────┐
│ providers││tools ││ skills  │
│ (LLM)    ││      ││         │
└──────────┘└──────┘└─────────┘
       │
       ▼
┌────────────────────────────────────┐
│          core (traits)             │
│  Agent | Message | Tool | Memory   │
└────────────────────────────────────┘
```

## 3. 核心 Trait 定义

### 3.1 Agent Trait

```rust
#[async_trait]
pub trait Agent: Send + Sync {
    async fn reply(&mut self, msg: Message) -> Result<Message, AgentError>;
    fn name(&self) -> &str;
    fn toolkit(&self) -> &Toolkit;
    fn memory(&self) -> &dyn Memory;
}
```

### 3.2 Message Types

```rust
pub struct Message {
    pub id: String,
    pub role: MessageRole,  // System | User | Assistant | Tool
    pub content: MessageContent,  // Text | Image | File | Mixed
    pub metadata: HashMap<String, Value>,
    pub timestamp: DateTime<Utc>,
}

pub enum ContentPart {
    Text { text: String },
    Image { url: String, caption: Option<String> },
    Video { url: String, caption: Option<String> },
    Audio { data: String },
    File { url: String, filename: String },
    Refusal { refusal: String },
}
```

### 3.3 Tool Trait

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;
    async fn execute(&self, params: Value) -> Result<String, ToolError>;
}
```

### 3.4 Channel Trait

```rust
pub trait Channel: Send + Sync {
    fn channel_type(&self) -> ChannelType;
    async fn start(&mut self) -> Result<(), ChannelError>;
    async fn stop(&mut self) -> Result<(), ChannelError>;
    async fn send_text(&mut self, to_handle: &str, text: &str, meta: Option<&Metadata>) -> Result<(), ChannelError>;
    async fn send_content_parts(&mut self, to_handle: &str, parts: Vec<ContentPart>, meta: Option<&Metadata>) -> Result<(), ChannelError>;
    fn resolve_session_id(&self, sender_id: &str, meta: Option<&Metadata>) -> String;
    fn clone_with_config(&self, config: &ChannelConfig) -> Result<Box<dyn Channel>, ChannelError>;
}
```

### 3.5 LLMProvider Trait

```rust
#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn chat_completion(&self, messages: Vec<Message>, tools: Option<Vec<ToolSchema>>, tool_choice: Option<ToolChoice>) -> Result<LLMResponse, LLMError>;
    async fn chat_completion_stream(&self, messages: Vec<Message>, tools: Option<Vec<ToolSchema>>) -> Result<Pin<Box<dyn Stream<Item = Result<String, LLMError>> + Send>>, LLMError>;
    fn count_tokens(&self, messages: &[Message]) -> usize;
}
```

### 3.6 Memory Trait

```rust
#[async_trait]
pub trait Memory: Send + Sync {
    async fn add(&mut self, msg: Message) -> Result<(), MemoryError>;
    fn get_all(&self) -> Vec<Message>;
    fn get_recent(&self, n: usize) -> Vec<Message>;
    async fn clear(&mut self) -> Result<(), MemoryError>;
    fn token_count(&self) -> usize;
}
```

## 4. 关键模块设计

### 4.1 ReAct 执行器

- 循环执行：推理 → 工具调用 → 结果反馈
- 最大迭代次数限制
- 工具执行错误处理

### 4.2 LLM Provider

- **OpenAI-compatible**: OpenAI, Claude, DeepSeek 等
- **Local**: llama.cpp, MLX, Ollama (通过 HTTP API)

### 4.3 Skills 系统

支持三种 skill 类型：
- **native**: Rust 动态库 (.so/.dylib/.dll)
- **wasm**: WebAssembly 沙箱
- **python**: 通过 Python bridge (兼容性)

### 4.4 配置管理

- 兼容现有 config.json 格式
- 热重载支持 (notify crate)
- 环境变量支持

### 4.5 内存管理

- 自动压缩 (超过阈值时)
- 保存总结到文件
- 支持 vector/keyword 搜索

### 4.6 定时任务

- Cron 表达式支持
- 优雅启动/停止
- 任务持久化

## 5. 渠道优先级

1. **Feishu** - 飞书 (最高优先级)
2. DingTalk - 钉钉
3. Discord
4. Telegram
5. QQ
6. iMessage

## 6. 技术栈

| 组件 | 选择 |
|------|------|
| 异步运行时 | Tokio |
| Web 框架 | Axum |
| 序列化 | Serde |
| HTTP 客户端 | Reqwest |
| 错误处理 | thiserror |
| 日志 | tracing |
| 文件监听 | notify |
| 定时任务 | cron + chrono |

## 7. 依赖映射

| Python | Rust |
|--------|------|
| FastAPI | Axum |
| Uvicorn | Tokio |
| APScheduler | cron + tokio::spawn |
| discord-py | serenity0 |
| dingtalk-stream | reqwest (HTTP client) |
| python-dotenv | dotenvy |
| Playwright | headless-chrome |

## 8. 兼容性保证

- API 契约与 Python 版本一致
- 前端无需修改
- 配置文件格式兼容
- 消息格式兼容 AgentScope

## 9. 构建和部署

```bash
# 开发构建
cargo build

# 发布构建
cargo build --release

# 单文件发布 (upx 压缩)
upx --best --lzma target/release/copaw
```

## 10. 风险和缓解

| 风险 | 缓解措施 |
|------|----------|
| 开发周期长 | 模块化并行开发，优先核心功能 |
| 生态不完整 | 通过 HTTP API 调用外部服务 |
| Python skills 兼容 | Python bridge / WASM |
| 学习曲线 | 渐进式迁移，保持 API 兼容 |
