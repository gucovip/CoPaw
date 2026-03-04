// -*- coding: utf-8 -*-
// -*- mode: rust -*-
//! Agent trait and related types for CoPaw agent system.
//!
//! This module defines the core Agent trait that all CoPaw agents must implement.
//! It provides the interface for agent-message interaction, tool management, and
//! memory handling.
//!
//! Based on the Python implementation:
//! - src/copaw/agents/react_agent.py - The main CoPawAgent
//! - AgentScope's ReActAgent base class

use crate::memory::Memory;
use crate::message::Message;
use crate::tool::Toolkit;
use async_trait::async_trait;

/// Error type for agent operations
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AgentError {
    /// Agent reply operation failed
    #[error("reply failed: {0}")]
    ReplyFailed(String),

    /// Tool execution failed during agent reasoning
    #[error("tool execution failed: {0}")]
    ToolExecutionFailed(String),

    /// Memory operation failed during agent processing
    #[error("memory operation failed: {0}")]
    MemoryFailed(String),

    /// Model invocation failed
    #[error("model invocation failed: {0}")]
    ModelFailed(String),

    /// Agent exceeded maximum iterations
    #[error("agent exceeded maximum iterations: {0}")]
    MaxIterationsExceeded(usize),

    /// Invalid agent state
    #[error("invalid agent state: {0}")]
    InvalidState(String),

    /// Agent configuration error
    #[error("configuration error: {0}")]
    ConfigurationError(String),

    /// Hook execution failed
    #[error("hook execution failed: {0}")]
    HookFailed(String),
}

/// Core trait that all agents must implement
///
/// This trait defines the interface for agents in the CoPaw system.
/// Agents are responsible for processing incoming messages and generating
/// responses using their tools and memory.
///
/// # Python Reference
///
/// Corresponds to AgentScope's `Agent` class and CoPaw's `CoPawAgent`.
/// See `src/copaw/agents/react_agent.py` for the Python implementation.
///
/// # Example
///
/// ```rust
/// use copaw_core::agent::{Agent, AgentError};
/// use copaw_core::message::Message;
/// use copaw_core::memory::{InMemoryMemory, Memory};
/// use copaw_core::tool::Toolkit;
///
/// struct MockAgent {
///     toolkit: Toolkit,
///     memory: InMemoryMemory,
/// }
///
/// #[async_trait::async_trait]
/// impl Agent for MockAgent {
///     fn name(&self) -> &str {
///         "mock"
///     }
///
///     fn toolkit(&self) -> &Toolkit {
///         &self.toolkit
///     }
///
///     fn memory(&self) -> &dyn Memory {
///         &self.memory
///     }
///
///     async fn reply(&mut self, msg: Message) -> Result<Message, AgentError> {
///         // In a real implementation, you would:
///         // 1. Add the message to memory
///         // 2. Process using tools if needed
///         // 3. Generate a response
///         Ok(Message::assistant("response"))
///     }
/// }
/// ```
#[async_trait]
pub trait Agent: Send + Sync {
    /// Returns the name of this agent
    ///
    /// This name is used to identify the agent in logs and when
    /// communicating with users.
    fn name(&self) -> &str;

    /// Returns a reference to this agent's toolkit
    ///
    /// The toolkit contains all the tools that the agent can use
    /// to perform actions.
    fn toolkit(&self) -> &Toolkit;

    /// Returns a reference to this agent's memory
    ///
    /// The memory stores the conversation history and is used
    /// to maintain context across multiple interactions.
    fn memory(&self) -> &dyn Memory;

    /// Process a message and generate a response
    ///
    /// This is the main entry point for agent interaction. The agent
    /// should process the incoming message, optionally use tools, and
    /// return a response message.
    ///
    /// # Parameters
    ///
    /// - `msg`: The incoming message to process
    ///
    /// # Returns
    ///
    /// A response message from the agent
    ///
    /// # Errors
    ///
    /// Returns an `AgentError` if the reply cannot be generated
    ///
    /// # Python Reference
    ///
    /// Corresponds to the `reply` method in `CoPawAgent` and
    /// AgentScope's `Agent.reply()`.
    async fn reply(&mut self, msg: Message) -> Result<Message, AgentError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::InMemoryMemory;

    // Mock tool for testing
    struct MockTool {
        name: &'static str,
    }

    #[async_trait::async_trait]
    impl crate::tool::Tool for MockTool {
        fn name(&self) -> &str {
            self.name
        }

        fn description(&self) -> &str {
            "A mock tool for testing"
        }

        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {}
            })
        }

        async fn execute(
            &self,
            _params: serde_json::Value,
        ) -> Result<String, crate::tool::ToolError> {
            Ok("mock tool executed".to_string())
        }
    }

    // Simple mock agent implementation for testing
    struct MockAgent {
        name: String,
        toolkit: Toolkit,
        memory: InMemoryMemory,
    }

    #[async_trait::async_trait]
    impl Agent for MockAgent {
        fn name(&self) -> &str {
            &self.name
        }

        fn toolkit(&self) -> &Toolkit {
            &self.toolkit
        }

        fn memory(&self) -> &dyn Memory {
            &self.memory
        }

        async fn reply(&mut self, msg: Message) -> Result<Message, AgentError> {
            // Add message to memory
            self.memory
                .add(msg.clone())
                .await
                .map_err(|e| AgentError::MemoryFailed(e.to_string()))?;

            // Return a simple response
            Ok(Message::assistant(format!("Response from {}", self.name)))
        }
    }

    impl MockAgent {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                toolkit: Toolkit::new(),
                memory: InMemoryMemory::new(),
            }
        }

        fn with_tool(name: &'static str) -> Self {
            let mut agent = Self::new("test");
            agent.toolkit.register(Box::new(MockTool { name })).unwrap();
            agent
        }
    }

    #[tokio::test]
    async fn test_agent_reply() {
        let mut agent = MockAgent::new("test");

        let response = agent.reply(Message::user("hello")).await.unwrap();

        assert_eq!(response.role(), &crate::message::MessageRole::Assistant);
        assert_eq!(response.content().as_text().unwrap(), "Response from test");
    }

    #[tokio::test]
    async fn test_agent_name() {
        let agent = MockAgent::new("friday");

        assert_eq!(agent.name(), "friday");
    }

    #[tokio::test]
    async fn test_agent_toolkit() {
        let agent = MockAgent::with_tool("mock_tool");

        assert!(agent.toolkit().get("mock_tool").is_some());
        assert_eq!(agent.toolkit().len(), 1);
    }

    #[tokio::test]
    async fn test_agent_reply_stores_in_memory() {
        let mut agent = MockAgent::new("test");

        let msg = Message::user("test message");
        agent.reply(msg).await.unwrap();

        let memory = agent.memory().get_all();
        assert_eq!(memory.len(), 1);
        assert_eq!(memory[0].content().as_text().unwrap(), "test message");
    }

    #[tokio::test]
    async fn test_agent_multiple_replies() {
        let mut agent = MockAgent::new("test");

        agent.reply(Message::user("first")).await.unwrap();
        agent.reply(Message::user("second")).await.unwrap();
        agent.reply(Message::user("third")).await.unwrap();

        let memory = agent.memory().get_all();
        assert_eq!(memory[0].content().as_text().unwrap(), "first");
        assert_eq!(memory[1].content().as_text().unwrap(), "second");
        assert_eq!(memory[2].content().as_text().unwrap(), "third");
    }

    #[tokio::test]
    async fn test_agent_error_display() {
        let err = AgentError::ReplyFailed("test error".to_string());
        assert!(err.to_string().contains("reply failed"));
        assert!(err.to_string().contains("test error"));

        let err = AgentError::ToolExecutionFailed("tool error".to_string());
        assert!(err.to_string().contains("tool execution failed"));

        let err = AgentError::MemoryFailed("mem error".to_string());
        assert!(err.to_string().contains("memory operation failed"));

        let err = AgentError::ModelFailed("model error".to_string());
        assert!(err.to_string().contains("model invocation failed"));

        let err = AgentError::MaxIterationsExceeded(100);
        assert!(err.to_string().contains("maximum iterations"));
        assert!(err.to_string().contains("100"));

        let err = AgentError::InvalidState("state error".to_string());
        assert!(err.to_string().contains("invalid agent state"));

        let err = AgentError::ConfigurationError("config error".to_string());
        assert!(err.to_string().contains("configuration error"));
    }

    #[tokio::test]
    async fn test_agent_empty_toolkit() {
        let agent = MockAgent::new("test");

        assert!(agent.toolkit().is_empty());
        assert_eq!(agent.toolkit().len(), 0);
    }

    #[tokio::test]
    async fn test_agent_memory_token_count() {
        let mut agent = MockAgent::new("test");

        agent.reply(Message::user("hello world")).await.unwrap();

        let token_count = agent.memory().token_count();
        assert!(token_count > 0);
    }

    #[tokio::test]
    async fn test_agent_get_recent_messages() {
        let mut agent = MockAgent::new("test");

        agent.reply(Message::user("first")).await.unwrap();
        agent.reply(Message::user("second")).await.unwrap();
        agent.reply(Message::user("third")).await.unwrap();

        let recent = agent.memory().get_recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].content().as_text().unwrap(), "second");
        assert_eq!(recent[1].content().as_text().unwrap(), "third");
    }
}
