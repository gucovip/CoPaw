//! ReAct Agent implementation for CoPaw.
//!
//! This module provides a ReAct-style agent that reasons about tasks,
//! uses tools, and maintains memory across conversations.
//!
//! Based on AgentScope's ReActAgent and CoPaw's CoPawAgent.

use async_trait::async_trait;
use copaw_core::agent::{Agent as CoreAgent, AgentError};
use copaw_core::llm::{LLMProvider, ToolChoice, ToolSchema};
use copaw_core::memory::{InMemoryMemory, Memory};
use copaw_core::message::{ContentPart, Message};
use copaw_core::tool::{Tool, ToolError, Toolkit};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Hook context passed to hook callbacks.
///
/// Contains information about the agent state at the time the hook is called.
#[derive(Debug, Clone)]
pub struct HookContext {
    /// The messages being processed
    pub messages: Vec<Message>,
    /// The agent name
    pub agent_name: String,
    /// Additional context data
    pub metadata: HashMap<String, String>,
}

impl HookContext {
    /// Create a new hook context.
    pub fn new(messages: Vec<Message>, agent_name: String) -> Self {
        Self {
            messages,
            agent_name,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the context.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Hook that runs before the LLM reasoning step.
///
/// Pre-reasoning hooks can:
/// - Modify the messages before they're sent to the LLM
/// - Check and trigger side effects (e.g., bootstrap guidance)
/// - Perform validation or logging
///
/// Based on AgentScope's pre-reasoning hooks and CoPaw's bootstrap/memory_compaction hooks.
#[async_trait]
pub trait PreReasoningHook: Send + Sync {
    /// Execute the hook before reasoning.
    ///
    /// # Parameters
    ///
    /// * `context` - The hook context containing agent state
    ///
    /// # Returns
    ///
    /// An optional modified version of the messages. If None, the original
    /// messages are used.
    ///
    /// # Errors
    ///
    /// Returns a HookError if the hook fails.
    async fn run(&self, context: HookContext) -> Result<Option<Vec<Message>>, HookError>;
}

/// Hook that runs after the LLM reasoning step.
///
/// Post-reasoning hooks can:
/// - Process the LLM's response
/// - Trigger side effects (e.g., memory compaction)
/// - Perform logging or monitoring
///
/// Based on AgentScope's post-reasoning hooks.
#[async_trait]
pub trait PostReasoningHook: Send + Sync {
    /// Execute the hook after reasoning.
    ///
    /// # Parameters
    ///
    /// * `context` - The hook context containing agent state
    /// * `response` - The LLM response message
    ///
    /// # Returns
    ///
    /// An optional modified version of the response. If None, the original
    /// response is used.
    ///
    /// # Errors
    ///
    /// Returns a HookError if the hook fails.
    async fn run(
        &self,
        context: HookContext,
        response: Message,
    ) -> Result<Option<Message>, HookError>;
}

/// Errors that can occur during hook execution.
#[derive(Debug, thiserror::Error)]
pub enum HookError {
    /// Hook execution failed.
    #[error("Hook execution failed: {0}")]
    ExecutionFailed(String),

    /// Hook returned invalid data.
    #[error("Invalid hook data: {0}")]
    InvalidData(String),
}

/// A boxed pre-reasoning hook that can be registered.
#[derive(Clone)]
pub struct BoxedPreReasoningHook {
    name: String,
    hook: Arc<dyn PreReasoningHook>,
}

impl BoxedPreReasoningHook {
    /// Create a new boxed hook.
    pub fn new(name: impl Into<String>, hook: Arc<dyn PreReasoningHook>) -> Self {
        Self {
            name: name.into(),
            hook,
        }
    }

    /// Get the hook name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Execute the hook.
    pub async fn run(&self, context: HookContext) -> Result<Option<Vec<Message>>, HookError> {
        self.hook.run(context).await
    }
}

/// A boxed post-reasoning hook that can be registered.
#[derive(Clone)]
pub struct BoxedPostReasoningHook {
    name: String,
    hook: Arc<dyn PostReasoningHook>,
}

impl BoxedPostReasoningHook {
    /// Create a new boxed hook.
    pub fn new(name: impl Into<String>, hook: Arc<dyn PostReasoningHook>) -> Self {
        Self {
            name: name.into(),
            hook,
        }
    }

    /// Get the hook name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Execute the hook.
    pub async fn run(
        &self,
        context: HookContext,
        response: Message,
    ) -> Result<Option<Message>, HookError> {
        self.hook.run(context, response).await
    }
}

/// Mock LLM provider for testing
#[derive(Debug, Clone)]
pub struct MockLLMProvider {
    /// Response to return
    pub response_text: String,
}

impl Default for MockLLMProvider {
    fn default() -> Self {
        Self {
            response_text: "Default response".to_string(),
        }
    }
}

impl MockLLMProvider {
    pub fn new(response: &str) -> Self {
        Self {
            response_text: response.to_string(),
        }
    }
}

#[async_trait]
impl LLMProvider for MockLLMProvider {
    async fn chat_completion(
        &self,
        _messages: Vec<Message>,
        _tools: Option<Vec<ToolSchema>>,
        _tool_choice: Option<ToolChoice>,
    ) -> Result<copaw_core::llm::LLMResponse, copaw_core::llm::LLMError> {
        Ok(copaw_core::llm::LLMResponse::new(
            Message::assistant(self.response_text.clone()),
            copaw_core::llm::Usage::new(10, 5),
        ))
    }

    fn count_tokens(&self, messages: &[Message]) -> usize {
        messages.len() * 10
    }
}

/// Mock tool for testing
#[derive(Debug, Clone)]
pub struct MockTool {
    pub name: &'static str,
    pub description: &'static str,
    pub response: String,
}

impl MockTool {
    pub fn new(name: &'static str, description: &'static str, response: &str) -> Self {
        Self {
            name,
            description,
            response: response.to_string(),
        }
    }
}

#[async_trait]
impl Tool for MockTool {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &str {
        self.description
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _params: serde_json::Value) -> Result<String, ToolError> {
        Ok(self.response.clone())
    }
}

/// ReAct-style agent with reasoning, tool use, and memory.
///
/// This agent implements the ReAct (Reasoning + Acting) paradigm:
/// 1. Reason about the task
/// 2. Act by calling tools if needed
/// 3. Observe tool results
/// 4. Repeat until completion
///
/// Based on:
/// - AgentScope's ReActAgent
/// - CoPaw's CoPawAgent from src/copaw/agents/react_agent.py
pub struct ReActAgent<P>
where
    P: LLMProvider + Send + Sync,
{
    /// Agent name
    name: String,

    /// LLM provider for generating responses
    model: Arc<P>,

    /// Toolkit containing available tools
    toolkit: Toolkit,

    /// Conversation memory
    memory: InMemoryMemory,

    /// Maximum number of reasoning iterations
    max_iters: usize,

    /// System prompt
    sys_prompt: String,

    /// Pre-reasoning hooks
    pre_reasoning_hooks: Mutex<Vec<BoxedPreReasoningHook>>,

    /// Post-reasoning hooks
    post_reasoning_hooks: Mutex<Vec<BoxedPostReasoningHook>>,
}

impl<P> ReActAgent<P>
where
    P: LLMProvider + Send + Sync,
{
    /// Create a new ReAct agent.
    ///
    /// # Arguments
    ///
    /// * `name` - The agent's name
    /// * `model` - The LLM provider to use
    /// * `sys_prompt` - The system prompt
    pub fn new<S: Into<String>>(name: S, model: Arc<P>, sys_prompt: S) -> Self {
        Self {
            name: name.into(),
            model,
            toolkit: Toolkit::new(),
            memory: InMemoryMemory::new(),
            max_iters: 50,
            sys_prompt: sys_prompt.into(),
            pre_reasoning_hooks: Mutex::new(Vec::new()),
            post_reasoning_hooks: Mutex::new(Vec::new()),
        }
    }

    /// Set the maximum number of iterations.
    pub fn with_max_iters(mut self, max_iters: usize) -> Self {
        self.max_iters = max_iters;
        self
    }

    /// Add a tool to the toolkit.
    pub fn add_tool(&mut self, tool: Box<dyn Tool>) -> Result<(), AgentError> {
        self.toolkit
            .register(tool)
            .map_err(|e| AgentError::ConfigurationError(e.to_string()))
    }

    /// Get the agent's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the agent's toolkit.
    pub fn toolkit(&self) -> &Toolkit {
        &self.toolkit
    }

    /// Get the agent's memory.
    pub fn memory(&self) -> &InMemoryMemory {
        &self.memory
    }

    /// Register a pre-reasoning hook.
    ///
    /// Pre-reasoning hooks are called before the LLM reasoning step.
    /// They can modify messages or trigger side effects.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for the hook
    /// * `hook` - The hook to register
    ///
    /// # Errors
    ///
    /// Returns an error if a hook with the same name already exists.
    pub fn register_pre_reasoning_hook(
        &mut self,
        name: impl Into<String>,
        hook: Arc<dyn PreReasoningHook>,
    ) -> Result<(), AgentError> {
        let name = name.into();
        let mut hooks = self
            .pre_reasoning_hooks
            .lock()
            .map_err(|e| AgentError::ConfigurationError(e.to_string()))?;

        if hooks.iter().any(|h| h.name() == name) {
            return Err(AgentError::ConfigurationError(format!(
                "Pre-reasoning hook '{}' already exists",
                name
            )));
        }

        hooks.push(BoxedPreReasoningHook::new(name, hook));
        Ok(())
    }

    /// Register a post-reasoning hook.
    ///
    /// Post-reasoning hooks are called after the LLM reasoning step.
    /// They can process the response or trigger side effects.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for the hook
    /// * `hook` - The hook to register
    ///
    /// # Errors
    ///
    /// Returns an error if a hook with the same name already exists.
    pub fn register_post_reasoning_hook(
        &mut self,
        name: impl Into<String>,
        hook: Arc<dyn PostReasoningHook>,
    ) -> Result<(), AgentError> {
        let name = name.into();
        let mut hooks = self
            .post_reasoning_hooks
            .lock()
            .map_err(|e| AgentError::ConfigurationError(e.to_string()))?;

        if hooks.iter().any(|h| h.name() == name) {
            return Err(AgentError::ConfigurationError(format!(
                "Post-reasoning hook '{}' already exists",
                name
            )));
        }

        hooks.push(BoxedPostReasoningHook::new(name, hook));
        Ok(())
    }

    /// Unregister a pre-reasoning hook by name.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the hook to remove
    ///
    /// # Returns
    ///
    /// `true` if the hook was found and removed, `false` otherwise.
    pub fn unregister_pre_reasoning_hook(&mut self, name: &str) -> bool {
        if let Ok(mut hooks) = self.pre_reasoning_hooks.lock() {
            let original_len = hooks.len();
            hooks.retain(|h| h.name() != name);
            hooks.len() < original_len
        } else {
            false
        }
    }

    /// Unregister a post-reasoning hook by name.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the hook to remove
    ///
    /// # Returns
    ///
    /// `true` if the hook was found and removed, `false` otherwise.
    pub fn unregister_post_reasoning_hook(&mut self, name: &str) -> bool {
        if let Ok(mut hooks) = self.post_reasoning_hooks.lock() {
            let original_len = hooks.len();
            hooks.retain(|h| h.name() != name);
            hooks.len() < original_len
        } else {
            false
        }
    }

    /// Get the number of registered pre-reasoning hooks.
    pub fn pre_reasoning_hook_count(&self) -> usize {
        self.pre_reasoning_hooks
            .lock()
            .map(|h| h.len())
            .unwrap_or(0)
    }

    /// Get the number of registered post-reasoning hooks.
    pub fn post_reasoning_hook_count(&self) -> usize {
        self.post_reasoning_hooks
            .lock()
            .map(|h| h.len())
            .unwrap_or(0)
    }

    /// Execute all pre-reasoning hooks.
    async fn run_pre_reasoning_hooks(
        &self,
        mut messages: Vec<Message>,
    ) -> Result<Vec<Message>, AgentError> {
        let hooks: Vec<BoxedPreReasoningHook> = {
            let lock = self
                .pre_reasoning_hooks
                .lock()
                .map_err(|e| AgentError::ConfigurationError(e.to_string()))?;
            lock.iter().cloned().collect()
        };

        let context = HookContext::new(messages.clone(), self.name.clone());

        for hook in hooks {
            match hook.run(context.clone()).await {
                Ok(Some(modified)) => messages = modified,
                Ok(None) => {} // No modification
                Err(e) => {
                    return Err(AgentError::HookFailed(format!(
                        "Pre-reasoning hook '{}' failed: {}",
                        hook.name(),
                        e
                    )));
                }
            }
        }

        Ok(messages)
    }

    /// Execute all post-reasoning hooks.
    async fn run_post_reasoning_hooks(&self, mut response: Message) -> Result<Message, AgentError> {
        let hooks: Vec<BoxedPostReasoningHook> = {
            let lock = self
                .post_reasoning_hooks
                .lock()
                .map_err(|e| AgentError::ConfigurationError(e.to_string()))?;
            lock.iter().cloned().collect()
        };

        let messages = self.memory.get_all();
        let context = HookContext::new(messages, self.name.clone());

        for hook in hooks {
            match hook.run(context.clone(), response.clone()).await {
                Ok(Some(modified)) => response = modified,
                Ok(None) => {} // No modification
                Err(e) => {
                    return Err(AgentError::HookFailed(format!(
                        "Post-reasoning hook '{}' failed: {}",
                        hook.name(),
                        e
                    )));
                }
            }
        }

        Ok(response)
    }

    /// Perform the reasoning step.
    async fn reason(&self, messages: Vec<Message>) -> Result<Message, AgentError> {
        // Run pre-reasoning hooks
        let messages = self.run_pre_reasoning_hooks(messages).await?;

        let has_tools = !self.toolkit.is_empty();
        let tool_choice = if has_tools {
            Some(ToolChoice::Auto)
        } else {
            None
        };

        let tools = if has_tools {
            Some(
                self.toolkit
                    .all()
                    .iter()
                    .filter_map(|name| self.toolkit.get(name))
                    .map(|tool| {
                        ToolSchema::from_parts(
                            tool.name(),
                            tool.description(),
                            tool.parameters_schema(),
                        )
                    })
                    .collect(),
            )
        } else {
            None
        };

        let response = self
            .model
            .chat_completion(messages, tools, tool_choice)
            .await
            .map_err(|e| AgentError::ModelFailed(e.to_string()))?;

        Ok(response.content().clone())
    }

    /// Execute a tool call.
    async fn execute_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<String, AgentError> {
        let tool = self.toolkit.get(tool_name).ok_or_else(|| {
            AgentError::ToolExecutionFailed(format!("Tool '{}' not found", tool_name))
        })?;

        tool.execute(arguments)
            .await
            .map_err(|e| AgentError::ToolExecutionFailed(e.to_string()))
    }

    /// Get messages for the LLM (system prompt + history).
    fn build_messages(&self, query: &Message) -> Vec<Message> {
        let mut messages = vec![Message::system(self.sys_prompt.clone())];
        messages.extend(self.memory.get_all());
        messages.push(query.clone());
        messages
    }
}

#[async_trait]
impl<P> CoreAgent for ReActAgent<P>
where
    P: LLMProvider + Send + Sync,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn toolkit(&self) -> &copaw_core::tool::Toolkit {
        &self.toolkit
    }

    fn memory(&self) -> &dyn Memory {
        &self.memory
    }

    async fn reply(&mut self, msg: Message) -> Result<Message, AgentError> {
        // Add user message to memory
        self.memory
            .add(msg.clone())
            .await
            .map_err(|e| AgentError::MemoryFailed(e.to_string()))?;

        // Build context messages
        let messages = self.build_messages(&msg);

        // Reason about the query
        let mut response = self.reason(messages).await?;

        // Handle tool calls if present
        let tool_calls: Vec<(String, String, serde_json::Value)> = response
            .content()
            .to_parts()
            .into_iter()
            .filter_map(|part| {
                if let ContentPart::ToolUse {
                    id, name, input, ..
                } = part
                {
                    Some((id, name, input))
                } else {
                    None
                }
            })
            .collect();

        if !tool_calls.is_empty() {
            // Add the assistant's tool call to memory
            self.memory
                .add(response.clone())
                .await
                .map_err(|e| AgentError::MemoryFailed(e.to_string()))?;

            // Execute each tool call
            for (id, name, input) in tool_calls {
                let tool_result = self.execute_tool(&name, input.clone()).await?;
                let tool_result_msg = Message::tool(vec![ContentPart::tool_result(
                    &id,
                    &name,
                    copaw_core::message::ToolOutput::text(tool_result),
                )]);

                self.memory
                    .add(tool_result_msg)
                    .await
                    .map_err(|e| AgentError::MemoryFailed(e.to_string()))?;
            }

            // Reason again with tool results
            let messages = self.build_messages(&msg);
            response = self.reason(messages).await?;
        }

        // Run post-reasoning hooks
        let response = self.run_post_reasoning_hooks(response).await?;

        // Add final response to memory
        self.memory
            .add(response.clone())
            .await
            .map_err(|e| AgentError::MemoryFailed(e.to_string()))?;

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_react_agent_creation() {
        let provider = Arc::new(MockLLMProvider::new("Test response"));
        let agent = ReActAgent::new("test_agent", provider, "You are a helpful assistant");
        assert_eq!(agent.name(), "test_agent");
        assert_eq!(agent.max_iters, 50);
    }

    #[test]
    fn test_react_agent_with_max_iters() {
        let provider = Arc::new(MockLLMProvider::new("Test"));
        let agent = ReActAgent::new("test", provider, "You are helpful").with_max_iters(100);
        assert_eq!(agent.max_iters, 100);
    }

    #[test]
    fn test_react_agent_add_tool() {
        let provider = Arc::new(MockLLMProvider::new("Test"));
        let mut agent = ReActAgent::new("test", provider, "You are helpful");

        let tool = MockTool::new("test_tool", "A test tool", "Tool executed");
        agent.add_tool(Box::new(tool)).unwrap();

        assert_eq!(agent.toolkit().len(), 1);
        assert!(agent.toolkit().get("test_tool").is_some());
    }

    #[tokio::test]
    async fn test_react_agent_reply_simple() {
        let provider = Arc::new(MockLLMProvider::new("Hello, world!"));
        let mut agent = ReActAgent::new("test", provider, "You are helpful");

        let msg = Message::user("Hello");
        let response = agent.reply(msg).await.unwrap();

        assert_eq!(response.role().as_str(), "assistant");
        assert_eq!(response.content().as_text().unwrap(), "Hello, world!");
    }

    #[tokio::test]
    async fn test_react_agent_memory_stores_messages() {
        let provider = Arc::new(MockLLMProvider::new("Response"));
        let mut agent = ReActAgent::new("test", provider, "You are helpful");

        let msg = Message::user("Test message");
        agent.reply(msg).await.unwrap();

        let memory = agent.memory().get_all();
        assert!(memory.len() >= 2); // User message + response
    }

    #[test]
    fn test_mock_tool_execute() {
        let tool = MockTool::new("test", "Test tool", "Result");

        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let result = tool.execute(serde_json::json!({})).await.unwrap();
            assert_eq!(result, "Result");
        });
    }

    #[tokio::test]
    async fn test_react_agent_build_messages() {
        let provider = Arc::new(MockLLMProvider::new("Test"));
        let agent = ReActAgent::new("test", provider, "You are helpful");

        let msg = Message::user("Hello");
        let messages = agent.build_messages(&msg);

        assert_eq!(messages.len(), 2); // System prompt + user message
        assert_eq!(messages[0].role().as_str(), "system");
        assert_eq!(messages[1].role().as_str(), "user");
    }

    #[tokio::test]
    async fn test_react_agent_build_messages_with_history() {
        let provider = Arc::new(MockLLMProvider::new("Test"));
        let mut agent = ReActAgent::new("test", provider, "You are helpful");

        // Add some history
        agent.memory.add(Message::user("First")).await.unwrap();
        agent
            .memory
            .add(Message::assistant("Response"))
            .await
            .unwrap();

        let msg = Message::user("Second");
        let messages = agent.build_messages(&msg);

        assert_eq!(messages.len(), 4); // System + 2 history + new message
    }

    #[tokio::test]
    async fn test_react_agent_toolkit_initially_empty() {
        let provider = Arc::new(MockLLMProvider::new("Test"));
        let agent = ReActAgent::new("test", provider, "You are helpful");

        assert!(agent.toolkit().is_empty());
        assert_eq!(agent.toolkit().len(), 0);
    }

    #[tokio::test]
    async fn test_react_agent_sys_prompt() {
        let provider = Arc::new(MockLLMProvider::new("Test"));
        let agent = ReActAgent::new(
            "test",
            provider,
            "You are a special assistant with specific instructions.",
        );

        let msg = Message::user("Hello");
        let messages = agent.build_messages(&msg);

        let system_msg = &messages[0];
        assert_eq!(system_msg.role().as_str(), "system");
        assert_eq!(
            system_msg.content().as_text().unwrap(),
            "You are a special assistant with specific instructions."
        );
    }

    #[tokio::test]
    async fn test_react_agent_multiple_replies() {
        let provider = Arc::new(MockLLMProvider::new("Response"));
        let mut agent = ReActAgent::new("test", provider, "You are helpful");

        agent.reply(Message::user("First")).await.unwrap();
        agent.reply(Message::user("Second")).await.unwrap();
        agent.reply(Message::user("Third")).await.unwrap();

        let memory = agent.memory().get_all();
        assert!(memory.len() >= 6); // 3 user + 3 assistant messages
    }

    // Hook tests

    /// Mock pre-reasoning hook for testing
    struct MockPreReasoningHook {
        /// Whether to modify messages
        modify: bool,
        /// Text to prepend to user messages
        prepend: String,
    }

    #[async_trait]
    impl PreReasoningHook for MockPreReasoningHook {
        async fn run(&self, context: HookContext) -> Result<Option<Vec<Message>>, HookError> {
            if !self.modify {
                return Ok(None);
            }

            let mut messages = context.messages.clone();
            // Find the first user message and prepend text
            for msg in messages.iter_mut() {
                if msg.role().as_str() == "user" {
                    let original = msg.content().as_text().unwrap_or_default();
                    let modified = format!("{} {}", self.prepend, original);
                    *msg = Message::user(modified);
                    break;
                }
            }
            Ok(Some(messages))
        }
    }

    /// Mock post-reasoning hook for testing
    struct MockPostReasoningHook {
        /// Whether to modify response
        modify: bool,
        /// Text to append to response
        append: String,
    }

    #[async_trait]
    impl PostReasoningHook for MockPostReasoningHook {
        async fn run(
            &self,
            _context: HookContext,
            response: Message,
        ) -> Result<Option<Message>, HookError> {
            if !self.modify {
                return Ok(None);
            }

            let original = response.content().as_text().unwrap_or_default();
            let modified = format!("{} {}", original, self.append);
            Ok(Some(Message::assistant(modified)))
        }
    }

    #[tokio::test]
    async fn test_register_pre_reasoning_hook() {
        let provider = Arc::new(MockLLMProvider::new("Response"));
        let mut agent = ReActAgent::new("test", provider, "You are helpful");

        let hook = Arc::new(MockPreReasoningHook {
            modify: false,
            prepend: "PREFACE:".to_string(),
        });

        assert!(agent
            .register_pre_reasoning_hook("test_hook", hook.clone())
            .is_ok());
        assert_eq!(agent.pre_reasoning_hook_count(), 1);

        // Should fail to register same hook twice
        assert!(agent
            .register_pre_reasoning_hook("test_hook", hook)
            .is_err());
        assert_eq!(agent.pre_reasoning_hook_count(), 1);
    }

    #[tokio::test]
    async fn test_register_post_reasoning_hook() {
        let provider = Arc::new(MockLLMProvider::new("Response"));
        let mut agent = ReActAgent::new("test", provider, "You are helpful");

        let hook = Arc::new(MockPostReasoningHook {
            modify: false,
            append: " [POST-HOOK]".to_string(),
        });

        assert!(agent
            .register_post_reasoning_hook("test_hook", hook.clone())
            .is_ok());
        assert_eq!(agent.post_reasoning_hook_count(), 1);

        // Should fail to register same hook twice
        assert!(agent
            .register_post_reasoning_hook("test_hook", hook)
            .is_err());
        assert_eq!(agent.post_reasoning_hook_count(), 1);
    }

    #[tokio::test]
    async fn test_unregister_pre_reasoning_hook() {
        let provider = Arc::new(MockLLMProvider::new("Response"));
        let mut agent = ReActAgent::new("test", provider, "You are helpful");

        let hook = Arc::new(MockPreReasoningHook {
            modify: false,
            prepend: "".to_string(),
        });

        agent
            .register_pre_reasoning_hook("hook1", hook.clone())
            .unwrap();
        agent
            .register_pre_reasoning_hook("hook2", hook.clone())
            .unwrap();
        assert_eq!(agent.pre_reasoning_hook_count(), 2);

        assert!(agent.unregister_pre_reasoning_hook("hook1"));
        assert_eq!(agent.pre_reasoning_hook_count(), 1);

        // Unregistering non-existent hook returns false
        assert!(!agent.unregister_pre_reasoning_hook("nonexistent"));
        assert_eq!(agent.pre_reasoning_hook_count(), 1);
    }

    #[tokio::test]
    async fn test_unregister_post_reasoning_hook() {
        let provider = Arc::new(MockLLMProvider::new("Response"));
        let mut agent = ReActAgent::new("test", provider, "You are helpful");

        let hook = Arc::new(MockPostReasoningHook {
            modify: false,
            append: "".to_string(),
        });

        agent
            .register_post_reasoning_hook("hook1", hook.clone())
            .unwrap();
        agent
            .register_post_reasoning_hook("hook2", hook.clone())
            .unwrap();
        assert_eq!(agent.post_reasoning_hook_count(), 2);

        assert!(agent.unregister_post_reasoning_hook("hook1"));
        assert_eq!(agent.post_reasoning_hook_count(), 1);

        // Unregistering non-existent hook returns false
        assert!(!agent.unregister_post_reasoning_hook("nonexistent"));
        assert_eq!(agent.post_reasoning_hook_count(), 1);
    }

    #[tokio::test]
    async fn test_pre_reasoning_hook_modifies_messages() {
        let provider = Arc::new(MockLLMProvider::new("Modified response"));
        let mut agent = ReActAgent::new("test", provider, "You are helpful");

        let hook = Arc::new(MockPreReasoningHook {
            modify: true,
            prepend: "[PRE-REASONING]".to_string(),
        });

        agent.register_pre_reasoning_hook("modifier", hook).unwrap();

        // The hook should have modified the user message
        // MockLLMProvider just returns its response_text, so we can't easily verify
        // that the message was modified, but we can verify the hook doesn't crash
        let result = agent.reply(Message::user("Hello")).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_post_reasoning_hook_modifies_response() {
        let provider = Arc::new(MockLLMProvider::new("Base response"));
        let mut agent = ReActAgent::new("test", provider, "You are helpful");

        let hook = Arc::new(MockPostReasoningHook {
            modify: true,
            append: "[POST-REASONING]".to_string(),
        });

        agent
            .register_post_reasoning_hook("modifier", hook)
            .unwrap();

        let response = agent.reply(Message::user("Hello")).await.unwrap();
        assert_eq!(
            response.content().as_text().unwrap(),
            "Base response [POST-REASONING]"
        );
    }

    #[tokio::test]
    async fn test_multiple_pre_reasoning_hooks() {
        let provider = Arc::new(MockLLMProvider::new("Response"));
        let mut agent = ReActAgent::new("test", provider, "You are helpful");

        let hook1 = Arc::new(MockPreReasoningHook {
            modify: false,
            prepend: "".to_string(),
        });
        let hook2 = Arc::new(MockPreReasoningHook {
            modify: false,
            prepend: "".to_string(),
        });
        let hook3 = Arc::new(MockPreReasoningHook {
            modify: false,
            prepend: "".to_string(),
        });

        agent.register_pre_reasoning_hook("hook1", hook1).unwrap();
        agent.register_pre_reasoning_hook("hook2", hook2).unwrap();
        agent.register_pre_reasoning_hook("hook3", hook3).unwrap();

        assert_eq!(agent.pre_reasoning_hook_count(), 3);

        let result = agent.reply(Message::user("Hello")).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_multiple_post_reasoning_hooks() {
        let provider = Arc::new(MockLLMProvider::new("Base"));
        let mut agent = ReActAgent::new("test", provider, "You are helpful");

        // Register hooks in order, but only one modifies
        let hook1 = Arc::new(MockPostReasoningHook {
            modify: false,
            append: "".to_string(),
        });
        let hook2 = Arc::new(MockPostReasoningHook {
            modify: true,
            append: "[HOOK2]".to_string(),
        });
        let hook3 = Arc::new(MockPostReasoningHook {
            modify: false,
            append: "".to_string(),
        });

        agent.register_post_reasoning_hook("hook1", hook1).unwrap();
        agent.register_post_reasoning_hook("hook2", hook2).unwrap();
        agent.register_post_reasoning_hook("hook3", hook3).unwrap();

        assert_eq!(agent.post_reasoning_hook_count(), 3);

        let response = agent.reply(Message::user("Hello")).await.unwrap();
        // Hook2 modifies the response, so we expect "[HOOK2]"
        assert_eq!(response.content().as_text().unwrap(), "Base [HOOK2]");
    }

    #[tokio::test]
    async fn test_hook_context() {
        let provider = Arc::new(MockLLMProvider::new("Response"));
        let mut agent = ReActAgent::new("test_agent", provider, "You are helpful");

        struct ContextCheckingHook {
            called: Arc<Mutex<bool>>,
        }

        #[async_trait]
        impl PreReasoningHook for ContextCheckingHook {
            async fn run(&self, context: HookContext) -> Result<Option<Vec<Message>>, HookError> {
                // Verify context has expected data
                assert_eq!(context.agent_name, "test_agent");
                assert!(!context.messages.is_empty());
                *self.called.lock().unwrap() = true;
                Ok(None)
            }
        }

        let called = Arc::new(Mutex::new(false));
        let hook = Arc::new(ContextCheckingHook {
            called: called.clone(),
        });

        agent.register_pre_reasoning_hook("checker", hook).unwrap();
        agent.reply(Message::user("Hello")).await.unwrap();

        assert!(*called.lock().unwrap());
    }

    #[tokio::test]
    async fn test_hook_error_propagates() {
        #[derive(Debug)]
        struct FailingHook;

        #[async_trait]
        impl PreReasoningHook for FailingHook {
            async fn run(&self, _context: HookContext) -> Result<Option<Vec<Message>>, HookError> {
                Err(HookError::ExecutionFailed(
                    "Hook failed intentionally".to_string(),
                ))
            }
        }

        let provider = Arc::new(MockLLMProvider::new("Response"));
        let mut agent = ReActAgent::new("test", provider, "You are helpful");

        agent
            .register_pre_reasoning_hook("failing", Arc::new(FailingHook))
            .unwrap();

        let result = agent.reply(Message::user("Hello")).await;
        assert!(result.is_err());

        match result {
            Err(AgentError::HookFailed(msg)) => {
                assert!(msg.contains("Pre-reasoning hook 'failing' failed"));
            }
            Err(e) => panic!("Expected HookFailed error, got: {:?}", e),
            Ok(_) => panic!("Expected error, got Ok"),
        }
    }

    #[tokio::test]
    async fn test_hook_context_with_metadata() {
        let provider = Arc::new(MockLLMProvider::new("Response"));
        let _agent = ReActAgent::new("test", provider, "You are helpful");

        let context = HookContext::new(vec![Message::user("test")], "agent1".to_string())
            .with_metadata("key1", "value1")
            .with_metadata("key2", "value2");

        assert_eq!(context.agent_name, "agent1");
        assert_eq!(context.messages.len(), 1);
        assert_eq!(context.metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(context.metadata.get("key2"), Some(&"value2".to_string()));
    }

    #[tokio::test]
    async fn test_agent_initially_has_no_hooks() {
        let provider = Arc::new(MockLLMProvider::new("Response"));
        let agent = ReActAgent::new("test", provider, "You are helpful");

        assert_eq!(agent.pre_reasoning_hook_count(), 0);
        assert_eq!(agent.post_reasoning_hook_count(), 0);
    }
}
