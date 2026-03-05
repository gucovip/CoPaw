// -*- coding: utf-8 -*-
// -*- mode: rust -*-
//! Integration tests for CoPaw Rust implementation.
//!
//! This test suite validates the integration between all major components
//! of the CoPaw system, including:
//! - Message serialization and deserialization
//! - Toolkit registration and tool retrieval
//! - Agent memory management
//! - LLM provider interactions
//! - Channel operations
//! - ReAct agent execution

use copaw_agents::react::{MockLLMProvider, MockTool, ReActAgent};
use copaw_core::agent::{Agent, AgentError};
use copaw_core::channel::{Channel, ChannelError, ChannelType, Metadata};
use copaw_core::llm::{LLMProvider, ToolChoice, ToolSchema, Usage};
use copaw_core::memory::{InMemoryMemory, Memory};
use copaw_core::message::{ContentPart, Message, MessageRole, ToolOutput};
use copaw_core::tool::{Tool, ToolError, Toolkit};
use copaw_tools::{FileTool, ShellTool};
use serde_json::json;
use std::str::FromStr;
use std::sync::Arc;
use tempfile::TempDir;

// ============================================================================
// Message Integration Tests
// ============================================================================

#[test]
fn test_message_serialization_roundtrip() {
    let msg = Message::user("test message");
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(msg.content(), parsed.content());
}

#[test]
fn test_message_with_parts_serialization() {
    let parts = vec![
        ContentPart::text("Hello"),
        ContentPart::thinking("Let me think about this"),
    ];
    let msg = Message::user(parts);
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(msg.content(), parsed.content());
}

#[test]
fn test_message_tool_use_serialization() {
    let tool_use = ContentPart::tool_use("call_123", "search", json!({"query": "test search"}));
    let msg = Message::assistant(vec![tool_use]);
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(msg.content(), parsed.content());
}

#[test]
fn test_message_tool_result_serialization() {
    let tool_result =
        ContentPart::tool_result("call_123", "search", ToolOutput::text("Search results"));
    let msg = Message::tool(vec![tool_result]);
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(msg.content(), parsed.content());
}

#[test]
fn test_message_metadata_serialization() {
    let msg = Message::user("test").with_metadata("key", json!("value"));
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(msg.get_metadata("key"), parsed.get_metadata("key"));
}

#[test]
fn test_message_all_roles() {
    let roles = vec![
        Message::system("system prompt"),
        Message::user("user message"),
        Message::assistant("assistant response"),
        Message::tool("tool result"),
    ];

    for msg in roles {
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(msg.role(), parsed.role());
        assert_eq!(msg.content(), parsed.content());
    }
}

// ============================================================================
// Toolkit Integration Tests
// ============================================================================

#[test]
fn test_toolkit_register_and_retrieve() {
    struct TestTool;

    #[async_trait::async_trait]
    impl Tool for TestTool {
        fn name(&self) -> &str {
            "test"
        }
        fn description(&self) -> &str {
            "Test tool"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            json!({})
        }
        async fn execute(&self, _: serde_json::Value) -> Result<String, ToolError> {
            Ok("test result".into())
        }
    }

    let mut toolkit = Toolkit::new();
    toolkit.register(Box::new(TestTool)).unwrap();

    let tool = toolkit.get("test").unwrap();
    assert_eq!(tool.name(), "test");
}

#[test]
fn test_toolkit_multiple_tools() {
    struct ToolA;
    struct ToolB;
    struct ToolC;

    #[async_trait::async_trait]
    impl Tool for ToolA {
        fn name(&self) -> &str {
            "tool_a"
        }
        fn description(&self) -> &str {
            "Tool A"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            json!({})
        }
        async fn execute(&self, _: serde_json::Value) -> Result<String, ToolError> {
            Ok("A".into())
        }
    }

    #[async_trait::async_trait]
    impl Tool for ToolB {
        fn name(&self) -> &str {
            "tool_b"
        }
        fn description(&self) -> &str {
            "Tool B"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            json!({})
        }
        async fn execute(&self, _: serde_json::Value) -> Result<String, ToolError> {
            Ok("B".into())
        }
    }

    #[async_trait::async_trait]
    impl Tool for ToolC {
        fn name(&self) -> &str {
            "tool_c"
        }
        fn description(&self) -> &str {
            "Tool C"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            json!({})
        }
        async fn execute(&self, _: serde_json::Value) -> Result<String, ToolError> {
            Ok("C".into())
        }
    }

    let mut toolkit = Toolkit::new();
    toolkit.register(Box::new(ToolA)).unwrap();
    toolkit.register(Box::new(ToolB)).unwrap();
    toolkit.register(Box::new(ToolC)).unwrap();

    assert_eq!(toolkit.len(), 3);
    assert_eq!(toolkit.all(), vec!["tool_a", "tool_b", "tool_c"]);
}

#[test]
fn test_toolkit_duplicate_registration_fails() {
    struct DuplicateTool;

    #[async_trait::async_trait]
    impl Tool for DuplicateTool {
        fn name(&self) -> &str {
            "duplicate"
        }
        fn description(&self) -> &str {
            "Duplicate tool"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            json!({})
        }
        async fn execute(&self, _: serde_json::Value) -> Result<String, ToolError> {
            Ok("result".into())
        }
    }

    let mut toolkit = Toolkit::new();
    toolkit.register(Box::new(DuplicateTool)).unwrap();
    let result = toolkit.register(Box::new(DuplicateTool));
    assert!(result.is_err());
}

// ============================================================================
// Memory Integration Tests
// ============================================================================

#[tokio::test]
async fn test_memory_add_and_retrieve() {
    let mut memory = InMemoryMemory::new();

    memory.add(Message::user("Hello")).await.unwrap();
    memory.add(Message::assistant("Hi there")).await.unwrap();

    let all = memory.get_all();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].content().as_text().unwrap(), "Hello");
    assert_eq!(all[1].content().as_text().unwrap(), "Hi there");
}

#[tokio::test]
async fn test_memory_conversation_flow() {
    let mut memory = InMemoryMemory::new();

    // Simulate a conversation
    memory
        .add(Message::user("What's the weather?"))
        .await
        .unwrap();
    memory
        .add(Message::assistant("I'll check the weather for you."))
        .await
        .unwrap();
    memory.add(Message::user("Thanks!")).await.unwrap();
    memory
        .add(Message::assistant("You're welcome!"))
        .await
        .unwrap();

    let messages = memory.get_all();
    assert_eq!(messages.len(), 4);

    // Check order is preserved
    assert_eq!(
        messages[0].content().as_text().unwrap(),
        "What's the weather?"
    );
    assert_eq!(messages[3].content().as_text().unwrap(), "You're welcome!");
}

#[tokio::test]
async fn test_memory_token_count_accumulates() {
    let mut memory = InMemoryMemory::new();

    let initial_count = memory.token_count();
    assert_eq!(initial_count, 0);

    memory.add(Message::user("Short message")).await.unwrap();
    let count1 = memory.token_count();
    assert!(count1 > 0);

    memory
        .add(Message::assistant(
            "This is a much longer response that should increase the token count",
        ))
        .await
        .unwrap();
    let count2 = memory.token_count();
    assert!(count2 > count1);
}

#[tokio::test]
async fn test_memory_get_recent() {
    let mut memory = InMemoryMemory::new();

    for i in 1..=10 {
        memory
            .add(Message::user(format!("Message {}", i)))
            .await
            .unwrap();
    }

    let recent = memory.get_recent(3);
    assert_eq!(recent.len(), 3);
    assert_eq!(recent[0].content().as_text().unwrap(), "Message 8");
    assert_eq!(recent[1].content().as_text().unwrap(), "Message 9");
    assert_eq!(recent[2].content().as_text().unwrap(), "Message 10");
}

#[tokio::test]
async fn test_memory_clear() {
    let mut memory = InMemoryMemory::new();

    memory.add(Message::user("test")).await.unwrap();
    assert_eq!(memory.len(), 1);

    memory.clear().await.unwrap();
    assert_eq!(memory.len(), 0);
    assert!(memory.is_empty());
}

// ============================================================================
// LLM Provider Integration Tests
// ============================================================================

#[tokio::test]
async fn test_llm_provider_chat_completion() {
    struct TestLLM;

    #[async_trait::async_trait]
    impl LLMProvider for TestLLM {
        async fn chat_completion(
            &self,
            _messages: Vec<Message>,
            _tools: Option<Vec<ToolSchema>>,
            _tool_choice: Option<ToolChoice>,
        ) -> Result<copaw_core::llm::LLMResponse, copaw_core::llm::LLMError> {
            Ok(copaw_core::llm::LLMResponse::new(
                Message::assistant("Test response"),
                Usage::new(10, 5),
            ))
        }

        fn count_tokens(&self, messages: &[Message]) -> usize {
            messages.len() * 10
        }
    }

    let llm = TestLLM;
    let response = llm
        .chat_completion(vec![Message::user("Hello")], None, None)
        .await
        .unwrap();

    assert_eq!(
        response.content().content().as_text().unwrap(),
        "Test response"
    );
    assert_eq!(response.usage().prompt_tokens, 10);
    assert_eq!(response.usage().completion_tokens, 5);
    assert_eq!(response.usage().total_tokens(), 15);
}

#[tokio::test]
async fn test_llm_provider_with_tools() {
    struct TestLLM;

    #[async_trait::async_trait]
    impl LLMProvider for TestLLM {
        async fn chat_completion(
            &self,
            _messages: Vec<Message>,
            tools: Option<Vec<ToolSchema>>,
            tool_choice: Option<ToolChoice>,
        ) -> Result<copaw_core::llm::LLMResponse, copaw_core::llm::LLMError> {
            // Verify tools were passed
            assert!(tools.is_some());
            assert_eq!(tools.as_ref().unwrap().len(), 1);
            assert_eq!(tools.as_ref().unwrap()[0].function().name(), "test_tool");

            // Verify tool choice
            assert_eq!(tool_choice, Some(ToolChoice::Auto));

            Ok(copaw_core::llm::LLMResponse::new(
                Message::assistant("Response with tools"),
                Usage::new(20, 10),
            ))
        }

        fn count_tokens(&self, messages: &[Message]) -> usize {
            messages.len() * 10
        }
    }

    let llm = TestLLM;
    let tools = vec![ToolSchema::from_parts(
        "test_tool",
        "A test tool",
        json!({"type": "object"}),
    )];

    let response = llm
        .chat_completion(
            vec![Message::user("Hello")],
            Some(tools),
            Some(ToolChoice::Auto),
        )
        .await
        .unwrap();

    assert_eq!(
        response.content().content().as_text().unwrap(),
        "Response with tools"
    );
}

#[tokio::test]
async fn test_llm_usage_calculation() {
    let usage = Usage::new(100, 50);
    assert_eq!(usage.prompt_tokens, 100);
    assert_eq!(usage.completion_tokens, 50);
    assert_eq!(usage.total_tokens(), 150);

    // Test with explicit total
    let usage = Usage {
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: Some(200),
    };
    assert_eq!(usage.total_tokens(), 200);
}

// ============================================================================
// Agent Integration Tests
// ============================================================================

#[tokio::test]
async fn test_agent_reply_simple() {
    let provider = Arc::new(MockLLMProvider::new("Hello!"));
    let mut agent = ReActAgent::new("test_agent", provider, "You are helpful");

    let response = agent.reply(Message::user("Hi")).await.unwrap();

    assert_eq!(response.role(), &MessageRole::Assistant);
    assert_eq!(response.content().as_text().unwrap(), "Hello!");
}

#[tokio::test]
async fn test_agent_memory_integration() {
    let provider = Arc::new(MockLLMProvider::new("Response"));
    let mut agent = ReActAgent::new("test_agent", provider, "You are helpful");

    // Send multiple messages
    agent.reply(Message::user("First message")).await.unwrap();
    agent.reply(Message::user("Second message")).await.unwrap();

    // Check memory contains conversation
    let memory = agent.memory().get_all();
    assert!(memory.len() >= 4); // 2 user + 2 assistant
}

#[tokio::test]
async fn test_agent_with_tools() {
    let provider = Arc::new(MockLLMProvider::new("Response"));
    let mut agent = ReActAgent::new("test_agent", provider, "You are helpful");

    // Add a tool
    let tool = MockTool::new("test_tool", "A test tool", "Tool result");
    agent.add_tool(Box::new(tool)).unwrap();

    // Verify tool is registered
    assert_eq!(agent.toolkit().len(), 1);
    assert!(agent.toolkit().get("test_tool").is_some());
}

#[tokio::test]
async fn test_agent_system_prompt() {
    let provider = Arc::new(MockLLMProvider::new("Response"));
    let agent = ReActAgent::new(
        "test_agent",
        provider,
        "You are a special assistant with custom instructions.",
    );

    // Verify agent name is set correctly
    assert_eq!(agent.name(), "test_agent");
}

#[tokio::test]
async fn test_agent_preserves_conversation_history() {
    let provider = Arc::new(MockLLMProvider::new("Response"));
    let mut agent = ReActAgent::new("test_agent", provider, "You are helpful");

    // First conversation
    agent.reply(Message::user("What is 2+2?")).await.unwrap();
    agent.reply(Message::user("What about 3+3?")).await.unwrap();

    // Check memory is preserved
    let memory = agent.memory().get_all();
    assert!(memory.len() >= 4); // At least 2 user + 2 assistant messages
}

// ============================================================================
// Built-in Tools Integration Tests
// ============================================================================

#[tokio::test]
async fn test_shell_tool_integration() {
    let tool = ShellTool::current_dir();

    let result = tool
        .execute(json!({"command": "echo hello"}))
        .await
        .unwrap();
    assert_eq!(result, "hello");
}

#[tokio::test]
async fn test_shell_tool_with_timeout() {
    let tool = ShellTool::current_dir();

    // Command that times out
    let result = tool
        .execute(json!({"command": "sleep 10", "timeout": 1}))
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_file_tool_integration() {
    let tmp = TempDir::new().unwrap();
    let tool = FileTool::new(tmp.path());

    // Write a file
    let write_result = tool
        .execute(json!({
            "operation": "write",
            "file_path": "test.txt",
            "content": "Hello, World!"
        }))
        .await
        .unwrap();
    assert!(write_result.contains("Wrote"));

    // Read the file
    let read_result = tool
        .execute(json!({
            "operation": "read",
            "file_path": "test.txt"
        }))
        .await
        .unwrap();
    assert_eq!(read_result, "Hello, World!");

    // Edit the file
    let edit_result = tool
        .execute(json!({
            "operation": "edit",
            "file_path": "test.txt",
            "old_text": "World",
            "new_text": "Rust"
        }))
        .await
        .unwrap();
    assert!(edit_result.contains("replaced"));

    // Verify edit
    let final_read = tool
        .execute(json!({
            "operation": "read",
            "file_path": "test.txt"
        }))
        .await
        .unwrap();
    assert_eq!(final_read, "Hello, Rust!");
}

// ============================================================================
// Channel Integration Tests
// ============================================================================

#[test]
fn test_channel_type_from_str() {
    assert_eq!(
        ChannelType::from_str("feishu").unwrap(),
        ChannelType::Feishu
    );
    assert_eq!(
        ChannelType::from_str("discord").unwrap(),
        ChannelType::Discord
    );
    assert_eq!(
        ChannelType::from_str("dingtalk").unwrap(),
        ChannelType::DingTalk
    );
    assert_eq!(ChannelType::from_str("qq").unwrap(), ChannelType::QQ);
    assert_eq!(
        ChannelType::from_str("telegram").unwrap(),
        ChannelType::Telegram
    );
    assert_eq!(
        ChannelType::from_str("imessage").unwrap(),
        ChannelType::IMessage
    );
    assert_eq!(
        ChannelType::from_str("console").unwrap(),
        ChannelType::Console
    );
}

#[test]
fn test_channel_type_display() {
    assert_eq!(ChannelType::Feishu.as_str(), "feishu");
    assert_eq!(ChannelType::Discord.as_str(), "discord");
    assert_eq!(format!("{}", ChannelType::DingTalk), "dingtalk");
}

struct MockChannel {
    channel_type: ChannelType,
}

#[async_trait::async_trait]
impl Channel for MockChannel {
    fn channel_type(&self) -> ChannelType {
        self.channel_type.clone()
    }

    async fn start(&mut self) -> Result<(), ChannelError> {
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ChannelError> {
        Ok(())
    }

    async fn send_text(
        &mut self,
        _to_handle: &str,
        _text: &str,
        _meta: Option<&Metadata>,
    ) -> Result<(), ChannelError> {
        Ok(())
    }

    async fn send_content_parts(
        &mut self,
        _to_handle: &str,
        _parts: Vec<ContentPart>,
        _meta: Option<&Metadata>,
    ) -> Result<(), ChannelError> {
        Ok(())
    }
}

#[tokio::test]
async fn test_channel_lifecycle() {
    let mut channel = MockChannel {
        channel_type: ChannelType::Feishu,
    };

    channel.start().await.unwrap();
    channel.stop().await.unwrap();
}

#[tokio::test]
async fn test_channel_resolve_session_id() {
    let channel = MockChannel {
        channel_type: ChannelType::Discord,
    };

    let session_id = channel.resolve_session_id("user123", None);
    assert_eq!(session_id, "discord:user123");

    let mut meta = Metadata::new();
    meta.insert(
        "conversation_id".to_string(),
        json!("oc_a0553eda9014c201e6969b4ad5de368c"),
    );

    let session_id = channel.resolve_session_id("user123", Some(&meta));
    assert_eq!(session_id, "discord:9b4ad5de368c");
}

#[tokio::test]
async fn test_channel_send_operations() {
    let mut channel = MockChannel {
        channel_type: ChannelType::Telegram,
    };

    channel.send_text("@user", "Hello!", None).await.unwrap();

    let parts = vec![
        ContentPart::text("Text message"),
        ContentPart::thinking("Thinking"),
    ];
    channel
        .send_content_parts("@user", parts, None)
        .await
        .unwrap();
}

// ============================================================================
// Error Handling Integration Tests
// ============================================================================

#[test]
fn test_error_display_messages() {
    let tool_err = ToolError::InvalidParameters("test error".to_string());
    assert!(tool_err.to_string().contains("Invalid parameters"));

    let agent_err = AgentError::ReplyFailed("failed".to_string());
    assert!(agent_err.to_string().contains("reply failed"));

    let channel_err = ChannelError::SendError("send failed".to_string());
    assert!(channel_err.to_string().contains("send failed"));
}

#[tokio::test]
async fn test_tool_error_propagation() {
    struct FailingTool;

    #[async_trait::async_trait]
    impl Tool for FailingTool {
        fn name(&self) -> &str {
            "failing"
        }
        fn description(&self) -> &str {
            "A tool that always fails"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            json!({})
        }
        async fn execute(&self, _: serde_json::Value) -> Result<String, ToolError> {
            Err(ToolError::ExecutionFailed(
                "Intentional failure".to_string(),
            ))
        }
    }

    let tool = FailingTool;
    let result = tool.execute(json!({})).await;
    assert!(result.is_err());
}

// ============================================================================
// End-to-End Integration Tests
// ============================================================================

#[tokio::test]
async fn test_end_to_end_conversation() {
    // Create a complete agent with tools
    let provider = Arc::new(MockLLMProvider::new("I can help with that!"));
    let mut agent = ReActAgent::new("copaw", provider, "You are CoPaw, a helpful AI assistant.");

    // Add some tools
    agent
        .add_tool(Box::new(MockTool::new(
            "search",
            "Search the web",
            "Search results",
        )))
        .unwrap();
    agent
        .add_tool(Box::new(MockTool::new("calculate", "Do math", "42")))
        .unwrap();

    // Simulate a conversation
    let response1 = agent.reply(Message::user("Hello CoPaw!")).await.unwrap();
    assert_eq!(response1.role(), &MessageRole::Assistant);

    let response2 = agent
        .reply(Message::user("What can you do?"))
        .await
        .unwrap();
    assert_eq!(response2.role(), &MessageRole::Assistant);

    // Verify memory accumulated
    let memory = agent.memory().get_all();
    assert!(memory.len() >= 4); // 2 user + 2 assistant

    // Verify tools are still registered
    assert_eq!(agent.toolkit().len(), 2);
}

#[tokio::test]
async fn test_agent_with_file_and_shell_tools() {
    let tmp = TempDir::new().unwrap();
    let provider = Arc::new(MockLLMProvider::new("Operations complete"));
    let mut agent = ReActAgent::new(
        "copaw",
        provider,
        "You are a helpful assistant with file and shell access.",
    );

    // Add built-in tools
    agent
        .add_tool(Box::new(ShellTool::new(tmp.path())))
        .unwrap();
    agent.add_tool(Box::new(FileTool::new(tmp.path()))).unwrap();

    // Verify tools are registered
    assert_eq!(agent.toolkit().len(), 2);
    assert!(agent.toolkit().get("execute_shell_command").is_some());
    assert!(agent.toolkit().get("file_operations").is_some());

    // Send a message
    let response = agent
        .reply(Message::user("Create a test file"))
        .await
        .unwrap();
    assert_eq!(response.role(), &MessageRole::Assistant);
}

#[tokio::test]
async fn test_multi_channel_session_resolution() {
    // Test session ID resolution across different channel types
    let channels = vec![
        MockChannel {
            channel_type: ChannelType::Feishu,
        },
        MockChannel {
            channel_type: ChannelType::Discord,
        },
        MockChannel {
            channel_type: ChannelType::Telegram,
        },
        MockChannel {
            channel_type: ChannelType::DingTalk,
        },
    ];

    for channel in channels {
        let session_id = channel.resolve_session_id("user123", None);
        assert!(session_id.contains("user123"));
        assert!(session_id.contains(&channel.channel_type().to_string()));
    }
}

#[tokio::test]
async fn test_message_builder_comprehensive() {
    // Test the message builder with all options
    let timestamp = chrono::Utc::now();
    let msg = Message::builder()
        .id("custom-id-123")
        .role(MessageRole::User)
        .content("Test message")
        .metadata("key1", json!("value1"))
        .metadata("key2", json!(42))
        .timestamp(timestamp)
        .build()
        .unwrap();

    assert_eq!(msg.id(), "custom-id-123");
    assert_eq!(msg.role(), &MessageRole::User);
    assert_eq!(msg.content().as_text().unwrap(), "Test message");
    assert_eq!(msg.get_metadata("key1"), Some(&json!("value1")));
    assert_eq!(msg.get_metadata("key2"), Some(&json!(42)));
}

// ============================================================================
// Performance and Stress Tests
// ============================================================================

#[tokio::test]
async fn test_memory_large_conversation() {
    let mut memory = InMemoryMemory::new();

    // Add many messages
    for i in 0..100 {
        memory
            .add(Message::user(format!("Message {}", i)))
            .await
            .unwrap();
        memory
            .add(Message::assistant(format!("Response {}", i)))
            .await
            .unwrap();
    }

    assert_eq!(memory.len(), 200);
    assert!(memory.token_count() > 0);

    // Test get_recent with large history
    let recent = memory.get_recent(10);
    assert_eq!(recent.len(), 10);
}

#[tokio::test]
async fn test_agent_long_conversation() {
    let provider = Arc::new(MockLLMProvider::new("Response"));
    let mut agent = ReActAgent::new("test", provider, "You are helpful");

    // Simulate a long conversation
    for i in 0..50 {
        agent
            .reply(Message::user(format!("Turn {}", i)))
            .await
            .unwrap();
    }

    let memory = agent.memory().get_all();
    assert!(memory.len() >= 100); // 50 user + 50 assistant
}

#[tokio::test]
async fn test_toolkit_many_tools() {
    let mut toolkit = Toolkit::new();

    // Register many tools
    for i in 0..50 {
        struct DynamicTool {
            name: String,
        }

        #[async_trait::async_trait]
        impl Tool for DynamicTool {
            fn name(&self) -> &str {
                &self.name
            }
            fn description(&self) -> &str {
                "Dynamic tool"
            }
            fn parameters_schema(&self) -> serde_json::Value {
                json!({})
            }
            async fn execute(&self, _: serde_json::Value) -> Result<String, ToolError> {
                Ok("result".into())
            }
        }

        let tool = DynamicTool {
            name: format!("tool_{}", i),
        };
        toolkit.register(Box::new(tool)).unwrap();
    }

    assert_eq!(toolkit.len(), 50);
    assert_eq!(toolkit.all().len(), 50);
}
