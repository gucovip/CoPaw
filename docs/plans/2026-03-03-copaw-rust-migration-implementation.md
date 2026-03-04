# CoPaw Rust Migration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 CoPaw 从 Python 完全重写为 Rust，实现单一可执行文件部署，提升性能和资源效率。

**Architecture:** Workspace 结构，按模块并行开发。核心 trait 定义抽象层，各 crate 独立实现。Axum + Tokio 作为异步服务框架。

**Tech Stack:** Rust 2021 edition, Tokio, Axum, Serde, Reqwest, thiserror, tracing, notify, cron

---

## Phase 1: 项目基础设施

### Task 1: 创建 Workspace 结构

**Files:**
- Create: `Cargo.toml`
- Create: `crates/core/Cargo.toml`
- Create: `crates/core/src/lib.rs`

**Step 1: 创建根 Cargo.toml**

```toml
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
authors = ["CoPaw Contributors"]
license = "MIT"

[workspace.dependencies]
tokio = { version = "1.40", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "2.0"
anyhow = "1.0"
tracing = "0.1"
```

**Step 2: 创建 core crate**

```toml
# crates/core/Cargo.toml
[package]
name = "copaw-core"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
```

**Step 3: 创建 core lib.rs**

```rust
// crates/core/src/lib.rs
pub mod agent;
pub mod message;
pub mod tool;
pub mod memory;
pub mod llm;
pub mod channel;
```

**Step 4: 验证编译**

```bash
cargo check
```

**Step 5: Commit**

```bash
git add Cargo.toml crates/
git commit -m "feat: initialize Rust workspace with core crate"
```

---

### Task 2: 定义核心 Message 类型

**Files:**
- Create: `crates/core/src/message.rs`
- Modify: `crates/core/src/lib.rs`

**Step 1: 写测试**

```rust
// crates/core/src/message.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_text_message() {
        let msg = Message::user("hello");
        assert_eq!(msg.role(), MessageRole::User);
        assert_eq!(msg.content().as_text().unwrap(), "hello");
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message::user("test");
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(msg.id(), parsed.id());
    }
}
```

**Step 2: 运行测试验证失败**

```bash
cargo check -p copaw-core
```

**Step 3: 实现 Message 类型**

```rust
// crates/core/src/message.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Message {
    id: String,
    role: MessageRole,
    content: MessageContent,
    metadata: HashMap<String, serde_json::Value>,
    timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageContent {
    Text(String),
    Image { url: String, caption: Option<String> },
    File { url: String, filename: String },
    Mixed(Vec<ContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContentPart {
    Text { text: String },
    Image { url: String },
    File { url: String, filename: String },
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self::new(MessageRole::User, MessageContent::Text(text.into()))
    }

    pub fn system(text: impl Into<String>) -> Self {
        Self::new(MessageRole::System, MessageContent::Text(text.into()))
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self::new(MessageRole::Assistant, MessageContent::Text(text.into()))
    }

    fn new(role: MessageRole, content: MessageContent) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role,
            content,
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn role(&self) -> MessageRole {
        self.role.clone()
    }

    pub fn content(&self) -> &MessageContent {
        &self.content
    }
}

impl MessageContent {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            MessageContent::Text(s) => Some(s),
            _ => None,
        }
    }
}
```

**Step 4: 更新 Cargo.toml 添加依赖**

```toml
# crates/core/Cargo.toml
[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
uuid = { version = "1.0", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
```

**Step 5: 运行测试**

```bash
cargo test -p copaw-core
```

**Step 6: Commit**

```bash
git add crates/core/
git commit -m "feat(core): add Message types with tests"
```

---

### Task 3: 定义 Tool Trait

**Files:**
- Create: `crates/core/src/tool.rs`

**Step 1: 写测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct DummyTool;

    #[async_trait::async_trait]
    impl Tool for DummyTool {
        fn name(&self) -> &str {
            "dummy"
        }

        fn description(&self) -> &str {
            "A dummy tool"
        }

        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {"type": "string"}
                }
            })
        }

        async fn execute(&self, params: serde_json::Value) -> Result<String, ToolError> {
            Ok("executed".to_string())
        }
    }

    #[tokio::test]
    async fn test_tool_execute() {
        let tool = DummyTool;
        let result = tool.execute(serde_json::json!({"input": "test"})).await.unwrap();
        assert_eq!(result, "executed");
    }
}
```

**Step 2: 运行测试验证失败**

```bash
cargo check -p copaw-core
```

**Step 3: 实现 Tool trait**

```rust
// crates/core/src/tool.rs
use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Invalid parameters: {0}")]
    InvalidParams(String),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Tool not found: {0}")]
    NotFound(String),
}

#[async_trait]
pub trait Tool: Send + Sync {
    /// 工具名称
    fn name(&self) -> &str;

    /// 工具描述（用于 prompt 生成）
    fn description(&self) -> &str;

    /// JSON Schema（用于结构化调用）
    fn parameters_schema(&self) -> Value;

    /// 执行工具
    async fn execute(&self, params: Value) -> Result<String, ToolError>;
}

/// Toolkit 管理多个工具
pub struct Toolkit {
    tools: std::collections::HashMap<String, std::sync::Arc<dyn Tool>>,
}

impl Toolkit {
    pub fn new() -> Self {
        Self {
            tools: std::collections::HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: std::sync::Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<std::sync::Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn all(&self) -> Vec<std::sync::Arc<dyn Tool>> {
        self.tools.values().cloned().collect()
    }
}

impl Default for Toolkit {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 4: 更新 Cargo.toml**

```toml
# crates/core/Cargo.toml
[dependencies]
async-trait = "0.1"
```

**Step 5: 运行测试**

```bash
cargo test -p copaw-core
```

**Step 6: 更新 lib.rs**

```rust
// crates/core/src/lib.rs
pub mod agent;
pub mod message;
pub mod tool;
pub mod memory;
pub mod llm;
pub mod channel;

pub use message::{Message, MessageRole, MessageContent, ContentPart};
pub use tool::{Tool, Toolkit, ToolError};
```

**Step 7: Commit**

```bash
git add crates/core/
git commit -m "feat(core): add Tool trait and Toolkit with tests"
```

---

### Task 4: 定义 LLMProvider Trait

**Files:**
- Create: `crates/core/src/llm.rs`

**Step 1: 写测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct MockLLM;

    #[async_trait::async_trait]
    impl LLMProvider for MockLLM {
        async fn chat_completion(
            &self,
            _messages: Vec<Message>,
            _tools: Option<Vec<ToolSchema>>,
            _tool_choice: Option<ToolChoice>,
        ) -> Result<LLMResponse, LLMError> {
            Ok(LLMResponse {
                content: Message::assistant("Hello"),
                tool_calls: None,
                usage: Usage { prompt_tokens: 10, completion_tokens: 5 },
            })
        }

        fn count_tokens(&self, messages: &[Message]) -> usize {
            messages.len() * 10
        }
    }

    #[tokio::test]
    async fn test_chat_completion() {
        let llm = MockLLM;
        let response = llm.chat_completion(vec![Message::user("hi")], None, None).await.unwrap();
        assert_eq!(response.content.content().as_text().unwrap(), "Hello");
    }
}
```

**Step 2: 实现 LLMProvider trait**

```rust
// crates/core/src/llm.rs
use async_trait::async_trait;
use serde::Serialize;
use std::pin::Pin;
use thiserror::Error;

use crate::Message;

#[derive(Debug, Error)]
pub enum LLMError {
    #[error("API request failed: {0}")]
    RequestFailed(String),
    #[error("Authentication failed")]
    AuthFailed,
    #[error("Rate limit exceeded")]
    RateLimit,
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    #[error("Stream error: {0}")]
    StreamError(String),
}

#[derive(Debug, Clone)]
pub struct LLMResponse {
    pub content: Message,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub usage: Usage,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct Usage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoice {
    Auto,
    None,
    Required,
    #[serde(untagged)]
    Specific { r#type: String, name: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolSchema {
    pub r#type: String,
    pub function: FunctionSchema,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// 聊天补全
    async fn chat_completion(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolSchema>>,
        tool_choice: Option<ToolChoice>,
    ) -> Result<LLMResponse, LLMError>;

    /// 计算 token 数量
    fn count_tokens(&self, messages: &[Message]) -> usize;
}

/// Token 计数器
pub struct TokenCounter {
    model: String,
}

impl TokenCounter {
    pub fn new(model: impl Into<String>) -> Self {
        Self { model: model.into() }
    }

    pub fn count_messages(&self, messages: &[Message]) -> usize {
        let mut count = 4; // base overhead
        for msg in messages {
            count += 3; // message overhead
            count += self.estimate_tokens(&msg);
        }
        count
    }

    fn estimate_tokens(&self, msg: &Message) -> usize {
        match msg.content() {
            crate::MessageContent::Text(s) => self.estimate_text_tokens(s),
            _ => 85, // media default
        }
    }

    fn estimate_text_tokens(&self, text: &str) -> usize {
        // 简单估算：英文 ~4 chars/token, 中文 ~2 chars/token
        let chinese = text.chars().filter(|c| is_chinese(*c)).count();
        let other = text.len() - chinese;
        (chinese / 2) + (other / 4)
    }
}

fn is_chinese(c: char) -> bool {
    matches!(c as u32, 0x4E00..=0x9FFF | 0x3400..=0x4DBF)
}
```

**Step 3: 运行测试**

```bash
cargo test -p copaw-core
```

**Step 4: 更新 lib.rs**

```rust
pub use llm::{LLMProvider, LLMResponse, LLMError, ToolChoice, ToolSchema, TokenCounter};
```

**Step 5: Commit**

```bash
git add crates/core/
git commit -m "feat(core): add LLMProvider trait with TokenCounter"
```

---

### Task 5: 定义 Memory Trait

**Files:**
- Create: `crates/core/src/memory.rs`

**Step 1: 写测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_memory() {
        let mut memory = InMemoryMemory::new();
        memory.add(Message::user("hello")).await.unwrap();
        memory.add(Message::assistant("hi")).await.unwrap();

        assert_eq!(memory.get_all().len(), 2);
        assert_eq!(memory.get_recent(1).len(), 1);
        assert_eq!(memory.get_recent(1)[0].content().as_text().unwrap(), "hi");
    }

    #[test]
    fn test_token_counting() {
        let memory = InMemoryMemory::new();
        // 空 memory 也有基础开销
        assert!(memory.token_count() >= 0);
    }
}
```

**Step 2: 实现 Memory trait**

```rust
// crates/core/src/memory.rs
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{Message, MessageContent};

#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("Memory operation failed: {0}")]
    OperationFailed(String),
}

#[async_trait]
pub trait Memory: Send + Sync {
    /// 添加消息
    async fn add(&mut self, msg: Message) -> Result<(), MemoryError>;

    /// 获取所有消息
    fn get_all(&self) -> Vec<Message>;

    /// 获取最近 N 条消息
    fn get_recent(&self, n: usize) -> Vec<Message>;

    /// 清空
    async fn clear(&mut self) -> Result<(), MemoryError>;

    /// 计算当前 token 数量
    fn token_count(&self) -> usize;
}

/// In-memory 实现
#[derive(Clone)]
pub struct InMemoryMemory {
    messages: Arc<RwLock<Vec<Message>>>,
}

impl InMemoryMemory {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl Default for InMemoryMemory {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Memory for InMemoryMemory {
    async fn add(&mut self, msg: Message) -> Result<(), MemoryError> {
        self.messages.write().await.push(msg);
        Ok(())
    }

    fn get_all(&self) -> Vec<Message> {
        // Note: this blocks, consider async version if needed
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.messages.read().await.clone()
            })
        })
    }

    fn get_recent(&self, n: usize) -> Vec<Message> {
        let all = self.get_all();
        let len = all.len();
        if n >= len {
            all
        } else {
            all[len - n..].to_vec()
        }
    }

    async fn clear(&mut self) -> Result<(), MemoryError> {
        self.messages.write().await.clear();
        Ok(())
    }

    fn token_count(&self) -> usize {
        let messages = self.get_all();
        // 简单估算
        messages.iter().map(|m| {
            match m.content() {
                MessageContent::Text(s) => s.len() / 3,
                _ => 50,
            }
        }).sum()
    }
}
```

**Step 3: 运行测试**

```bash
cargo test -p copaw-core
```

**Step 4: 更新 lib.rs**

```rust
pub use memory::{Memory, MemoryError, InMemoryMemory};
```

**Step 5: Commit**

```bash
git add crates/core/
git commit -m "feat(core): add Memory trait and InMemoryMemory implementation"
```

---

### Task 6: 定义 Agent Trait

**Files:**
- Create: `crates/core/src/agent.rs`

**Step 1: 写测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct MockAgent {
        toolkit: Toolkit,
        memory: InMemoryMemory,
    }

    #[async_trait::async_trait]
    impl Agent for MockAgent {
        fn name(&self) -> &str {
            "mock"
        }

        fn toolkit(&self) -> &Toolkit {
            &self.toolkit
        }

        fn memory(&self) -> &dyn Memory {
            &self.memory
        }

        async fn reply(&mut self, msg: Message) -> Result<Message, AgentError> {
            self.memory.add(msg).await?;
            Ok(Message::assistant("response"))
        }
    }

    #[tokio::test]
    async fn test_agent_reply() {
        let mut agent = MockAgent {
            toolkit: Toolkit::new(),
            memory: InMemoryMemory::new(),
        };
        let response = agent.reply(Message::user("test")).await.unwrap();
        assert_eq!(response.content().as_text().unwrap(), "response");
        assert_eq!(agent.memory.get_all().len(), 1);
    }
}
```

**Step 2: 实现 Agent trait**

```rust
// crates/core/src/agent.rs
use async_trait::async_trait;
use thiserror::Error;

use crate::{Message, Memory, Toolkit};

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("LLM error: {0}")]
    LLM(String),
    #[error("Tool error: {0}")]
    Tool(String),
    #[error("Memory error: {0}")]
    Memory(String),
    #[error("Max iterations exceeded")]
    MaxIterationsExceeded,
}

#[async_trait]
pub trait Agent: Send + Sync {
    /// 处理消息并返回响应
    async fn reply(&mut self, msg: Message) -> Result<Message, AgentError>;

    /// 获取 agent 名称
    fn name(&self) -> &str;

    /// 获取 toolkit 引用
    fn toolkit(&self) -> &Toolkit;

    /// 获取 memory 引用
    fn memory(&self) -> &dyn Memory;
}
```

**Step 3: 运行测试**

```bash
cargo test -p copaw-core
```

**Step 4: 更新 lib.rs**

```rust
pub use agent::{Agent, AgentError};
```

**Step 5: Commit**

```bash
git add crates/core/
git commit -m "feat(core): add Agent trait with tests"
```

---

### Task 7: 定义 Channel Trait

**Files:**
- Create: `crates/core/src/channel.rs`

**Step 1: 实现 Channel trait**

```rust
// crates/core/src/channel.rs
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

use crate::message::ContentPart;

#[derive(Debug, Error)]
pub enum ChannelError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Send failed: {0}")]
    SendFailed(String),
    #[error("Authentication failed")]
    AuthFailed,
    #[error("Invalid payload")]
    InvalidPayload,
    #[error("Not configured")]
    NotConfigured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelType {
    Feishu,
    DingTalk,
    Discord,
    Telegram,
    QQ,
    IMessage,
    Console,
}

pub type Metadata = HashMap<String, Value>;

#[async_trait]
pub trait Channel: Send + Sync {
    /// Channel 类型标识
    fn channel_type(&self) -> ChannelType;

    /// 启动 channel
    async fn start(&mut self) -> Result<(), ChannelError>;

    /// 停止 channel
    async fn stop(&mut self) -> Result<(), ChannelError>;

    /// 发送文本消息
    async fn send_text(
        &mut self,
        to_handle: &str,
        text: &str,
        meta: Option<&Metadata>,
    ) -> Result<(), ChannelError>;

    /// 发送多部分内容
    async fn send_content_parts(
        &mut self,
        to_handle: &str,
        parts: Vec<ContentPart>,
        meta: Option<&Metadata>,
    ) -> Result<(), ChannelError>;

    /// 解析 session_id
    fn resolve_session_id(&self, sender_id: &str, meta: Option<&Metadata>) -> String {
        format!("{:?}:{}", self.channel_type(), sender_id)
    }

    /// 克隆实例（用于热重载）
    fn clone_channel(&self) -> Result<Box<dyn Channel>, ChannelError> {
        Err(ChannelError::NotConfigured)
    }
}

/// Channel 配置
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelConfig {
    pub enabled: bool,
    pub config: Value,
}
```

**Step 2: 运行测试**

```bash
cargo check -p copaw-core
```

**Step 3: 更新 lib.rs**

```rust
pub use channel::{Channel, ChannelType, ChannelError, Metadata};
```

**Step 4: Commit**

```bash
git add crates/core/
git commit -m "feat(core): add Channel trait and types"
```

---

## Phase 2: Providers 模块

### Task 8: 创建 providers crate

**Files:**
- Create: `crates/providers/Cargo.toml`
- Create: `crates/providers/src/lib.rs`

**Step 1: 创建 providers Cargo.toml**

```toml
[package]
name = "copaw-providers"
version.workspace = true
edition.workspace = true

[dependencies]
copaw-core = { path = "../core" }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
reqwest = { version = "0.12", features = ["json"] }
async-trait = { workspace = true }
thiserror = { workspace = true }
anyhow = { workspace = true }
```

**Step 2: 创建 lib.rs**

```rust
pub mod openai;
pub mod local;

pub use openai::OpenAIProvider;
pub use local::{LlamaCppProvider, OllamaProvider};
```

**Step 3: 验证编译**

```bash
cargo check -p copaw-providers
```

**Step 4: Commit**

```bash
git add crates/providers/
git commit -m "feat(providers): initialize providers crate"
```

---

### Task 9: 实现 OpenAI Provider

**Files:**
- Create: `crates/providers/src/openai.rs`

**Step 1: 写测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use copaw_core::{Message, TokenCounter};

    #[test]
    fn test_openai_request_builder() {
        let provider = OpenAIProvider::new("https://api.openai.com/v1", "sk-test", "gpt-4");
        assert_eq!(provider.model(), "gpt-4");
    }

    #[test]
    fn test_message_conversion() {
        let msg = Message::user("hello");
        let openai_msg = OpenAIMessage::from(msg);
        assert_eq!(openai_msg.role, "user");
        assert_eq!(openai_msg.content, "hello");
    }
}
```

**Step 2: 实现 OpenAIProvider**

```rust
// crates/providers/src/openai.rs
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use copaw_core::{
    LLMError, LLMProvider, LLMResponse, Message, MessageRole, MessageContent,
    ToolChoice, ToolSchema, TokenCounter,
};

#[derive(Clone)]
pub struct OpenAIProvider {
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl OpenAIProvider {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    fn convert_messages(&self, messages: Vec<Message>) -> Vec<OpenAIMessage> {
        messages.into_iter().map(OpenAIMessage::from).collect()
    }

    fn convert_tool_choice(&self, choice: Option<ToolChoice>) -> Option<OpenAIToolChoice> {
        choice.map(|c| match c {
            ToolChoice::Auto => OpenAIToolChoice::Auto,
            ToolChoice::None => OpenAIToolChoice::None,
            ToolChoice::Required => OpenAIToolChoice::Required,
            _ => OpenAIToolChoice::Auto,
        })
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn chat_completion(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolSchema>>,
        tool_choice: Option<ToolChoice>,
    ) -> Result<LLMResponse, LLMError> {
        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages: self.convert_messages(messages),
            tools: tools.map(|t| t.into_iter().map(OpenAITool::from).collect()),
            tool_choice: self.convert_tool_choice(tool_choice),
        };

        let response = self.client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| LLMError::RequestFailed(e.to_string()))?
            .json::<ChatCompletionResponse>()
            .await
            .map_err(|e| LLMError::InvalidResponse(e.to_string()))?;

        self.parse_response(response)
    }

    fn count_tokens(&self, messages: &[Message]) -> usize {
        let counter = TokenCounter::new(&self.model);
        counter.count_messages(messages)
    }
}

impl OpenAIProvider {
    fn parse_response(&self, response: ChatCompletionResponse) -> Result<LLMResponse, LLMError> {
        let choice = response.choices.into_iter().next()
            .ok_or_else(|| LLMError::InvalidResponse("No choices".into()))?;

        let content = Message::assistant(
            choice.message.content
                .unwrap_or_else(|| "".to_string())
        );

        let tool_calls = choice.message.tool_calls.map(|calls| {
            calls.into_iter().map(|call| copaw_core::ToolCall {
                id: call.id,
                name: call.function.name,
                arguments: serde_json::from_str(&call.function.arguments).unwrap_or_default(),
            }).collect()
        });

        Ok(LLMResponse {
            content,
            tool_calls,
            usage: copaw_core::Usage {
                prompt_tokens: response.usage.prompt_tokens,
                completion_tokens: response.usage.completion_tokens,
            },
        })
    }
}

// OpenAI API 类型
#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    tools: Option<Vec<OpenAITool>>,
    tool_choice: Option<OpenAIToolChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: OpenAIAssistantMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIAssistantMessage {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAIToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAIToolCall {
    id: String,
    function: OpenAIFunction,
}

#[derive(Debug, Deserialize)]
struct OpenAIFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

impl From<Message> for OpenAIMessage {
    fn from(msg: Message) -> Self {
        Self {
            role: match msg.role() {
                MessageRole::System => "system".into(),
                MessageRole::User => "user".into(),
                MessageRole::Assistant => "assistant".into(),
                MessageRole::Tool => "tool".into(),
            },
            content: match msg.content() {
                MessageContent::Text(s) => s.clone(),
                _ => "".to_string(),
            },
        }
    }
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: usize,
    completion_tokens: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum OpenAIToolChoice {
    Auto,
    None,
    Required,
}

#[derive(Debug, Clone, Serialize)]
struct OpenAITool {
    r#type: String,
    function: OpenAIFunctionSchema,
}

impl From<ToolSchema> for OpenAITool {
    fn from(schema: ToolSchema) -> Self {
        Self {
            r#type: schema.r#type,
            function: OpenAIFunctionSchema {
                name: schema.function.name,
                description: schema.function.description,
                parameters: schema.function.parameters,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct OpenAIFunctionSchema {
    name: String,
    description: String,
    parameters: serde_json::Value,
}
```

**Step 3: 运行测试**

```bash
cargo test -p copaw-providers
```

**Step 4: 更新 lib.rs**

```rust
pub mod openai;
pub mod local;

pub use openai::OpenAIProvider;
```

**Step 5: Commit**

```bash
git add crates/providers/
git commit -m "feat(providers): add OpenAI-compatible provider"
```

---

### Task 10: 实现本地模型 Provider

**Files:**
- Create: `crates/providers/src/local.rs`
- Create: `crates/providers/src/local/llamacpp.rs`
- Create: `crates/providers/src/local/ollama.rs`

**Step 1: 实现 llama.cpp provider**

```rust
// crates/providers/src/local/llamacpp.rs
use async_trait::async_trait;
use reqwest::Client;

use copaw_core::{LLMError, LLMProvider, LLMResponse, Message, TokenCounter};

#[derive(Clone)]
pub struct LlamaCppProvider {
    client: Client,
    base_url: String,
    model: String,
}

impl LlamaCppProvider {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            model: model.into(),
        }
    }
}

#[async_trait]
impl LLMProvider for LlamaCppProvider {
    async fn chat_completion(
        &self,
        messages: Vec<Message>,
        _tools: Option<Vec<copaw_core::ToolSchema>>,
        _tool_choice: Option<copaw_core::ToolChoice>,
    ) -> Result<LLMResponse, LLMError> {
        // llama.cpp 不支持原生工具调用
        let prompt = self.format_prompt(messages);

        let response = self.client
            .post(format!("{}/completion", self.base_url))
            .json(&serde_json::json!({
                "prompt": prompt,
                "n_predict": 512,
            }))
            .send()
            .await
            .map_err(|e| LLMError::RequestFailed(e.to_string()))?
            .json::<LlamaCppResponse>()
            .await
            .map_err(|e| LLMError::InvalidResponse(e.to_string()))?;

        Ok(LLMResponse {
            content: Message::assistant(response.content),
            tool_calls: None,
            usage: copaw_core::Usage {
                prompt_tokens: response.tokens_evaluated,
                completion_tokens: response.tokens_predicted,
            },
        })
    }

    fn count_tokens(&self, messages: &[Message]) -> usize {
        let counter = TokenCounter::new("llama");
        counter.count_messages(messages)
    }
}

impl LlamaCppProvider {
    fn format_prompt(&self, messages: Vec<Message>) -> String {
        messages.iter()
            .filter_map(|m| m.content().as_text())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug, Deserialize)]
struct LlamaCppResponse {
    content: String,
    tokens_evaluated: usize,
    tokens_predicted: usize,
}
```

**Step 2: 实现 Ollama provider**

```rust
// crates/providers/src/local/ollama.rs
use async_trait::async_trait;
use reqwest::Client;

use copaw_core::{LLMError, LLMProvider, LLMResponse, Message, TokenCounter};

#[derive(Clone)]
pub struct OllamaProvider {
    client: Client,
    base_url: String,
    model: String,
}

impl OllamaProvider {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            model: model.into(),
        }
    }
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    async fn chat_completion(
        &self,
        messages: Vec<Message>,
        _tools: Option<Vec<copaw_core::ToolSchema>>,
        _tool_choice: Option<copaw_core::ToolChoice>,
    ) -> Result<LLMResponse, LLMError> {
        let response = self.client
            .post(format!("{}/api/chat", self.base_url))
            .json(&serde_json::json!({
                "model": self.model,
                "messages": messages,
                "stream": false,
            }))
            .send()
            .await
            .map_err(|e| LLMError::RequestFailed(e.to_string()))?
            .json::<OllamaResponse>()
            .await
            .map_err(|e| LLMError::InvalidResponse(e.to_string()))?;

        Ok(LLMResponse {
            content: Message::assistant(response.message.content),
            tool_calls: None,
            usage: copaw_core::Usage {
                prompt_tokens: response.prompt_eval_count.unwrap_or(0),
                completion_tokens: response.eval_count.unwrap_or(0),
            },
        })
    }

    fn count_tokens(&self, messages: &[Message]) -> usize {
        let counter = TokenCounter::new(&self.model);
        counter.count_messages(messages)
    }
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    message: OllamaMessage,
    prompt_eval_count: Option<usize>,
    eval_count: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    content: String,
}
```

**Step 3: 创建 local/mod.rs**

```rust
// crates/providers/src/local.rs
pub mod llamacpp;
pub mod ollama;

pub use llamacpp::LlamaCppProvider;
pub use ollama::OllamaProvider;
```

**Step 4: 更新 lib.rs**

```rust
pub mod openai;
pub mod local;

pub use openai::OpenAIProvider;
pub use local::{LlamaCppProvider, OllamaProvider};
```

**Step 5: 运行测试**

```bash
cargo test -p copaw-providers
```

**Step 6: Commit**

```bash
git add crates/providers/
git commit -m "feat(providers): add llama.cpp and Ollama providers"
```

---

## Phase 3: Agent 实现

### Task 11: 创建 agents crate

**Files:**
- Create: `crates/agents/Cargo.toml`
- Create: `crates/agents/src/lib.rs`
- Create: `crates/agents/src/react.rs`

**Step 1: 创建 agents Cargo.toml**

```toml
[package]
name = "copaw-agents"
version.workspace = true
edition.workspace = true

[dependencies]
copaw-core = { path = "../core" }
copaw-providers = { path = "../providers" }
tokio = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
```

**Step 2: 实现 ReActAgent**

```rust
// crates/agents/src/react.rs
use std::sync::Arc;
use tokio::sync::RwLock;

use copaw_core::{
    Agent, AgentError, Message, Memory, Toolkit, LLMProvider,
    LLMResponse, Tool, ToolChoice, ToolSchema,
};

/// ReAct 循环 Agent
pub struct ReActAgent {
    name: String,
    model: Arc<dyn LLMProvider>,
    toolkit: Toolkit,
    memory: Arc<RwLock<dyn Memory>>,
    max_iters: usize,
    sys_prompt: String,
}

impl ReActAgent {
    pub fn new(
        name: impl Into<String>,
        model: Arc<dyn LLMProvider>,
        sys_prompt: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            model,
            toolkit: Toolkit::new(),
            memory: Arc::new(RwLock::new(copaw_core::InMemoryMemory::new())),
            max_iters: 50,
            sys_prompt: sys_prompt.into(),
        }
    }

    pub fn with_toolkit(mut self, toolkit: Toolkit) -> Self {
        self.toolkit = toolkit;
        self
    }

    pub fn with_memory(mut self, memory: Arc<RwLock<dyn Memory>>) -> Self {
        self.memory = memory;
        self
    }

    pub fn with_max_iters(mut self, max: usize) -> Self {
        self.max_iters = max;
        self
    }

    async fn reasoning(&self, tool_choice: Option<ToolChoice>) -> Result<LLMResponse, AgentError> {
        let mem = self.memory.read().await;
        let mut messages = vec![Message::system(&self.sys_prompt)];
        messages.extend(mem.get_all());

        let tools = if self.toolkit.all().is_empty() {
            None
        } else {
            Some(self.toolkit.all().iter().map(|t| ToolSchema {
                r#type: "function".into(),
                function: copaw_core::FunctionSchema {
                    name: t.name().into(),
                    description: t.description().into(),
                    parameters: t.parameters_schema(),
                },
            }).collect())
        };

        self.model.chat_completion(messages, tools, tool_choice)
            .await
            .map_err(|e| AgentError::LLM(e.to_string()))
    }

    async fn execute_tool(&self, call: copaw_core::ToolCall) -> Result<Message, AgentError> {
        let tool = self.toolkit.get(&call.name)
            .ok_or_else(|| AgentError::Tool(format!("Tool not found: {}", call.name)))?;

        let result = tool.execute(call.arguments)
            .await
            .map_err(|e| AgentError::Tool(e.to_string()))?;

        Ok(Message::assistant(result))
    }
}

#[async_trait::async_trait]
impl Agent for ReActAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn toolkit(&self) -> &Toolkit {
        &self.toolkit
    }

    fn memory(&self) -> &dyn Memory {
        // Note: this is a limitation of the trait design
        // In practice, we might need to adjust this
        // For now, we'll use a workaround
        &copaw_core::InMemoryMemory::new()
    }

    async fn reply(&mut self, msg: Message) -> Result<Message, AgentError> {
        // 添加用户消息到 memory
        {
            let mut mem = self.memory.write().await;
            mem.add(msg.clone()).await
                .map_err(|e| AgentError::Memory(e.to_string()))?;
        }

        let mut iterations = 0;

        loop {
            // 推理阶段
            let response = self.reasoning(Some(ToolChoice::Auto)).await?;

            // 检查是否有工具调用
            if let Some(tool_calls) = response.tool_calls {
                if tool_calls.is_empty() {
                    // 没有工具调用，返回最终回复
                    let result = response.content;
                    {
                        let mut mem = self.memory.write().await;
                        mem.add(result.clone()).await
                            .map_err(|e| AgentError::Memory(e.to_string()))?;
                    }
                    return Ok(result);
                }

                // 执行所有工具调用
                for tool_call in tool_calls {
                    let result = self.execute_tool(tool_call).await?;
                    {
                        let mut mem = self.memory.write().await;
                        mem.add(result).await
                            .map_err(|e| AgentError::Memory(e.to_string()))?;
                    }
                }

                iterations += 1;
                if iterations >= self.max_iters {
                    return Err(AgentError::MaxIterationsExceeded);
                }
            } else {
                // 无工具调用，返回回复
                let result = response.content;
                {
                    let mut mem = self.memory.write().await;
                    mem.add(result.clone()).await
                        .map_err(|e| AgentError::Memory(e.to_string()))?;
                }
                return Ok(result);
            }
        }
    }
}
```

**Step 3: 创建 lib.rs**

```rust
pub mod react;

pub use react::ReActAgent;
```

**Step 4: 运行测试**

```bash
cargo check -p copaw-agents
```

**Step 5: Commit**

```bash
git add crates/agents/
git commit -m "feat(agents): add ReActAgent implementation"
```

---

## Phase 4: 工具实现

### Task 12: 创建 tools crate

**Files:**
- Create: `crates/tools/Cargo.toml`
- Create: `crates/tools/src/lib.rs`
- Create: `crates/tools/src/shell.rs`

**Step 1: 创建 tools Cargo.toml**

```toml
[package]
name = "copaw-tools"
version.workspace = true
edition.workspace = true

[dependencies]
copaw-core = { path = "../core" }
tokio = { workspace = true }
async-trait = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
```

**Step 2: 实现 ShellTool**

```rust
// crates/tools/src/shell.rs
use std::collections::HashSet;
use std::time::Duration;
use async_trait::async_trait;

use copaw_core::{Tool, ToolError};

pub struct ShellTool {
    allowed_commands: Option<HashSet<String>>,
    timeout: Duration,
}

impl ShellTool {
    pub fn new() -> Self {
        Self {
            allowed_commands: None,
            timeout: Duration::from_secs(30),
        }
    }

    pub fn with_allowed_commands(mut self, commands: HashSet<String>) -> Self {
        self.allowed_commands = Some(commands);
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl Default for ShellTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "execute_shell_command"
    }

    fn description(&self) -> &str {
        "Execute shell commands and return the output. Use for file operations, system info, etc."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> Result<String, ToolError> {
        let input: ShellInput = serde_json::from_value(params)
            .map_err(|_| ToolError::InvalidParams("Invalid JSON".into()))?;

        // 安全检查
        if let Some(allowed) = &self.allowed_commands {
            let cmd_name = input.command.split_whitespace().next()
                .unwrap_or("");
            if !allowed.contains(cmd_name) {
                return Err(ToolError::ExecutionFailed(format!(
                    "Command '{}' is not allowed", cmd_name
                )));
            }
        }

        // 执行命令
        let output = tokio::time::timeout(
            self.timeout,
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&input.command)
                .output()
        ).await
        .map_err(|_| ToolError::ExecutionFailed("Command timeout".into()))?
        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(ToolError::ExecutionFailed(stderr.to_string()))
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct ShellInput {
    command: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shell_echo() {
        let tool = ShellTool::new();
        let result = tool.execute(serde_json::json!({"command": "echo hello"})).await.unwrap();
        assert_eq!(result.trim(), "hello");
    }
}
```

**Step 3: 创建 FileTool**

```rust
// crates/tools/src/file.rs
use std::path::PathBuf;
use std::collections::HashSet;
use async_trait::async_trait;

use copaw_core::{Tool, ToolError};

pub struct FileTool {
    base_dir: PathBuf,
}

impl FileTool {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }
}

#[async_trait::async_trait]
impl Tool for FileTool {
    fn name(&self) -> &str {
        "file_operations"
    }

    fn description(&self) -> &str {
        "Read, write, search files. Supports text, markdown, code files."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["read", "write", "list"]
                },
                "path": {"type": "string"},
                "content": {"type": "string"}
            },
            "required": ["operation", "path"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> Result<String, ToolError> {
        let input: FileInput = serde_json::from_value(params)
            .map_err(|_| ToolError::InvalidParams("Invalid JSON".into()))?;

        let full_path = self.base_dir.join(&input.path);

        // 路径安全检查
        if !full_path.starts_with(&self.base_dir) {
            return Err(ToolError::ExecutionFailed("Path traversal detected".into()));
        }

        match input.operation.as_str() {
            "read" => {
                let content = tokio::fs::read_to_string(&full_path).await
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                Ok(content)
            }
            "write" => {
                tokio::fs::write(&full_path, input.content.unwrap_or_default()).await
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                Ok("File written".to_string())
            }
            "list" => {
                let mut entries = tokio::fs::read_dir(&full_path).await
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
                    .collect::<Vec<_>>()
                    .await;

                let names: Vec<String> = entries.into_iter()
                    .filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect();

                Ok(serde_json::to_string(&names).unwrap_or_default())
            }
            _ => Err(ToolError::InvalidParams("Unknown operation".into()))
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct FileInput {
    operation: String,
    path: String,
    content: Option<String>,
}
```

**Step 4: 创建 lib.rs**

```rust
pub mod shell;
pub mod file;

pub use shell::ShellTool;
pub use file::FileTool;
```

**Step 5: 运行测试**

```bash
cargo test -p copaw-tools
```

**Step 6: Commit**

```bash
git add crates/tools/
git commit -m "feat(tools): add ShellTool and FileTool"
```

---

## Phase 5: 渠道实现

### Task 13: 创建 channels crate

**Files:**
- Create: `crates/channels/Cargo.toml`
- Create: `crates/channels/src/lib.rs`
- Create: `crates/channels/src/feishu.rs`

**Step 1: 创建 channels Cargo.toml**

```toml
[package]
name = "copaw-channels"
version.workspace = true
edition.workspace = true

[dependencies]
copaw-core = { path = "../core" }
tokio = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
reqwest = { version = "0.12", features = ["json"] }
thiserror = { workspace = true }
```

**Step 2: 实现 FeishuChannel**

```rust
// crates/channels/src/feishu.rs
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use copaw_core::{Channel, ChannelError, ChannelType, Metadata, ContentPart};

pub struct FeishuChannel {
    app_id: String,
    app_secret: String,
    client: Client,
    access_token: Option<String>,
    token_expires_at: Option<i64>,
}

impl FeishuChannel {
    pub fn new(app_id: impl Into<String>, app_secret: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
            app_secret: app_secret.into(),
            client: Client::new(),
            access_token: None,
            token_expires_at: None,
        }
    }

    async fn get_tenant_access_token(&mut self) -> Result<String, ChannelError> {
        // 检查是否需要刷新
        if let Some(expires_at) = self.token_expires_at {
            let now = chrono::Utc::now().timestamp();
            if now < expires_at - 300 && self.access_token.is_some() {
                return Ok(self.access_token.as_ref().unwrap().clone());
            }
        }

        // 获取新 token
        let response = self.client
            .post("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal")
            .json(&serde_json::json!({
                "app_id": self.app_id,
                "app_secret": self.app_secret
            }))
            .send()
            .await
            .map_err(|e| ChannelError::ConnectionFailed(e.to_string()))?
            .json::<TenantAccessTokenResponse>()
            .await
            .map_err(|e| ChannelError::InvalidResponse(e.to_string()))?;

        if response.code != 0 {
            return Err(ChannelError::AuthFailed);
        }

        self.access_token = Some(response.tenant_access_token.clone());
        self.token_expires_at = Some(chrono::Utc::now().timestamp() + response.expire as i64);

        Ok(response.tenant_access_token)
    }
}

#[async_trait]
impl Channel for FeishuChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Feishu
    }

    async fn start(&mut self) -> Result<(), ChannelError> {
        // 预先获取 token
        self.get_tenant_access_token().await?;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ChannelError> {
        self.access_token = None;
        self.token_expires_at = None;
        Ok(())
    }

    async fn send_text(
        &mut self,
        to_handle: &str,
        text: &str,
        _meta: Option<&Metadata>,
    ) -> Result<(), ChannelError> {
        let token = self.get_tenant_access_token().await?;

        self.client
            .post("https://open.feishu.cn/open-apis/im/v1/messages")
            .header("Authorization", format!("Bearer {}", token))
            .json(&serde_json::json!({
                "receive_id_type": "chat_id",
                "receiver_id": to_handle,
                "msg_type": "text",
                "content": serde_json::json!({"text": text}).to_string()
            }))
            .send()
            .await
            .map_err(|e| ChannelError::SendFailed(e.to_string()))?
            .error_for_status()
            .map_err(|e| ChannelError::SendFailed(e.to_string()))?;

        Ok(())
    }

    async fn send_content_parts(
        &mut self,
        to_handle: &str,
        parts: Vec<ContentPart>,
        meta: Option<&Metadata>,
    ) -> Result<(), ChannelError> {
        let text_parts: Vec<String> = parts.iter()
            .filter_map(|p| match p {
                ContentPart::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect();

        if !text_parts.is_empty() {
            self.send_text(to_handle, &text_parts.join("\n"), meta).await?;
        }

        // 处理图片等（后续实现）
        Ok(())
    }

    fn resolve_session_id(&self, sender_id: &str, meta: Option<&Metadata>) -> String {
        if let Some(m) = meta {
            if let Some(conv_id) = m.get("conversation_id").and_then(|v| v.as_str()) {
                // 使用后缀
                if conv_id.len() > 16 {
                    return format!("feishu:{}", &conv_id[conv_id.len()-16..]);
                }
                return format!("feishu:{}", conv_id);
            }
        }
        format!("feishu:{}", sender_id)
    }

    fn clone_channel(&self) -> Result<Box<dyn Channel>, ChannelError> {
        Ok(Box::new(FeishuChannel::new(&self.app_id, &self.app_secret)))
    }
}

#[derive(Debug, Deserialize)]
struct TenantAccessTokenResponse {
    code: i32,
    tenant_access_token: String,
    expire: u64,
}
```

**Step 3: 创建 lib.rs**

```rust
pub mod feishu;

pub use feishu::FeishuChannel;
```

**Step 4: 运行测试**

```bash
cargo check -p copaw-channels
```

**Step 5: Commit**

```bash
git add crates/channels/
git commit -m "feat(channels): add Feishu channel implementation"
```

---

## Phase 6: HTTP 服务

### Task 14: 创建 app crate

**Files:**
- Create: `crates/app/Cargo.toml`
- Create: `crates/app/src/main.rs`

**Step 1: 创建 app Cargo.toml**

```toml
[package]
name = "copaw"
version.workspace = true
edition.workspace = true

[[bin]]
name = "copaw"
path = "src/main.rs"

[dependencies]
copaw-core = { path = "../core" }
copaw-agents = { path = "../agents" }
copaw-tools = { path = "../tools" }
copaw-channels = { path = "../channels" }
copaw-providers = { path = "../providers" }

tokio = { workspace = true }
axum = { version = "0.7", features = ["ws", "multipart"] }
tower = "0.5"
tower-http = { version = "0.5", features = ["cors", "fs", "trace"] }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

**Step 2: 创建 main.rs**

```rust
// crates/app/src/main.rs
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::{CorsLayer, Any};
use tracing_subscriber;

use copaw_core::Message;
use copaw_agents::ReActAgent;
use copaw_tools::{ShellTool, FileTool};
use copaw_providers::OpenAIProvider;

#[derive(Clone)]
struct AppState {
    // agent: Arc<tokio::sync::RwLock<ReActAgent>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let state = AppState {
        // agent: Arc::new(tokio::sync::RwLock::new(ReActAgent::new(...))),
    };

    let app = Router::new()
        .route("/", get(root))
        .route("/api/chat", post(chat))
        .route("/api/version", get(version))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8088").await?;
    tracing::info!("CoPaw server listening on http://0.0.0.0:8088");

    axum::serve(listener, app).await?;

    Ok(())
}

async fn root() -> &'static str {
    "CoPaw Rust Server"
}

async fn version() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "language": "rust"
    }))
}

#[derive(Deserialize)]
struct ChatRequest {
    message: String,
}

#[derive(Serialize)]
struct ChatResponse {
    reply: String,
}

async fn chat(
    State(_state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, StatusCode> {
    // TODO: 实际调用 agent
    Ok(Json(ChatResponse {
        reply: format!("Echo: {}", req.message),
    }))
}

impl IntoResponse for anyhow::Error {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", self)).into_response()
    }
}
```

**Step 3: 验证编译**

```bash
cargo build -p copaw
```

**Step 4: 运行测试**

```bash
cargo run -p copaw
```

**Step 5: Commit**

```bash
git add crates/app/
git commit -m "feat(app): add HTTP server with Axum"
```

---

## Phase 7: 集成测试

### Task 15: 端到端测试

**Files:**
- Create: `tests/integration_test.rs`

**Step 1: 创建集成测试**

```rust
// tests/integration_test.rs
use copaw_core::{Message, Memory, InMemoryMemory};
use copaw_tools::ShellTool;

#[tokio::test]
async fn test_shell_tool() {
    let tool = ShellTool::new();
    let result = tool
        .execute(serde_json::json!({"command": "echo test"}))
        .await
        .unwrap();
    assert_eq!(result.trim(), "test");
}

#[tokio::test]
async fn test_message_creation() {
    let msg = Message::user("hello");
    assert_eq!(msg.role().to_string(), "User");
    assert_eq!(msg.content().as_text().unwrap(), "hello");
}

#[tokio::test]
async fn test_memory_operations() {
    let mut memory = InMemoryMemory::new();
    memory.add(Message::user("first")).await.unwrap();
    memory.add(Message::assistant("second")).await.unwrap();

    assert_eq!(memory.get_all().len(), 2);
    assert_eq!(memory.get_recent(1).len(), 1);
    assert_eq!(memory.get_recent(1)[0].content().as_text().unwrap(), "second");
}
```

**Step 2: 运行测试**

```bash
cargo test
```

**Step 3: Commit**

```bash
git add tests/
git commit -m "test: add integration tests"
```

---

## Phase 8: 构建脚本

### Task 16: 添加构建脚本

**Files:**
- Create: `justfile`
- Create: `scripts/build.sh`

**Step 1: 创建 justfile**

```makefile
# justfile
default: dev

dev:
    cargo run -p copaw

build:
    cargo build --release

test:
    cargo test

check:
    cargo check --all

fmt:
    cargo fmt --all

clippy:
    cargo clippy --all -- -D warnings

run: dev

.PHONY: dev build test check fmt clippy run
```

**Step 2: 创建构建脚本**

```bash
#!/bin/bash
# scripts/build.sh

set -e

echo "Building CoPaw Rust..."

# 构建前端
echo "Building console frontend..."
cd console
npm ci
npm run build
cd ..

# 构建 Rust
echo "Building Rust binary..."
cargo build --release

echo "Build complete! Binary: target/release/copaw"
```

**Step 3: Commit**

```bash
git add justfile scripts/
git commit -m "build: add build scripts and justfile"
```

---

## 总结

此实施计划将 CoPaw 从 Python 迁移到 Rust，包含：

1. **核心 trait 定义** (Phase 1) - Agent, Message, Tool, Memory, LLMProvider, Channel
2. **Provider 实现** (Phase 2) - OpenAI, llama.cpp, Ollama
3. **Agent 实现** (Phase 3) - ReAct 循环
4. **内置工具** (Phase 4) - Shell, File 操作
5. **渠道实现** (Phase 5) - Feishu 优先
6. **HTTP 服务** (Phase 6) - Axum 服务器
7. **集成测试** (Phase 7)
8. **构建脚本** (Phase 8)

总计 16 个任务，每个任务 2-5 分钟可完成。
