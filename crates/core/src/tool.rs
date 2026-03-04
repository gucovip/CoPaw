//! Tool trait and toolkit for managing agent tools.
//!
//! This module provides the core abstraction for tools that can be used by
//! the CoPaw agent, inspired by AgentScope's tool system.
//!
//! # Python Reference
//!
//! - `src/copaw/agents/tools/` - Python tool implementations
//! - AgentScope's `ToolResponse` and `Toolkit` classes

use serde_json::Value;
use std::collections::HashMap;

/// Error type for tool execution failures.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    /// Invalid parameters provided to the tool.
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),

    /// Tool execution failed.
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    /// Tool not found in toolkit.
    #[error("Tool not found: {0}")]
    NotFound(String),

    /// Tool registration failed (e.g., duplicate name).
    #[error("Registration failed: {0}")]
    RegistrationFailed(String),

    /// I/O error during tool execution.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Core trait that all tools must implement.
///
/// This trait defines the interface for tools that can be used by the CoPaw agent.
/// Tools are async functions that take JSON parameters and return a string result.
///
/// # Python Reference
///
/// Corresponds to AgentScope's tool functions that return `ToolResponse`.
/// See `src/copaw/agents/tools/` for examples.
///
/// # Example
///
/// ```rust
/// use copaw_core::tool::{Tool, ToolError};
/// use serde_json::json;
///
/// struct DummyTool;
///
/// #[async_trait::async_trait]
/// impl Tool for DummyTool {
///     fn name(&self) -> &str {
///         "dummy"
///     }
///
///     fn description(&self) -> &str {
///         "A dummy tool"
///     }
///
///     fn parameters_schema(&self) -> serde_json::Value {
///         json!({
///             "type": "object",
///             "properties": {
///                 "input": {"type": "string"}
///             }
///         })
///     }
///
///     async fn execute(&self, params: serde_json::Value) -> Result<String, ToolError> {
///         Ok("executed".to_string())
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// Returns the unique name of this tool.
    ///
    /// This name is used to identify the tool in the toolkit and in agent messages.
    fn name(&self) -> &str;

    /// Returns a human-readable description of what this tool does.
    ///
    /// This description is used by the LLM to understand when and how to use the tool.
    fn description(&self) -> &str;

    /// Returns the JSON Schema for the tool's parameters.
    ///
    /// This schema is used to validate parameters and to inform the LLM about
    /// what parameters the tool accepts.
    ///
    /// # Python Reference
    ///
    /// In Python, this is derived from function signatures and docstrings.
    /// AgentScope tools use type hints to generate schemas.
    fn parameters_schema(&self) -> Value;

    /// Executes the tool with the given parameters.
    ///
    /// # Parameters
    ///
    /// - `params`: JSON value containing the tool parameters, validated against
    ///            the schema returned by `parameters_schema()`.
    ///
    /// # Returns
    ///
    /// A string result that will be included in the agent's response.
    ///
    /// # Python Reference
    ///
    /// Corresponds to Python tool functions that return `ToolResponse`.
    /// See `execute_shell_command` in `src/copaw/agents/tools/shell.py` for an example.
    async fn execute(&self, params: Value) -> Result<String, ToolError>;
}

/// Toolkit for managing multiple tools.
///
/// The toolkit stores tools by name and provides methods to register, retrieve,
/// and list tools. This is used by the agent to discover and execute tools.
///
/// # Python Reference
///
/// Corresponds to AgentScope's `Toolkit` class used in `react_agent.py`.
///
/// # Example
///
/// ```rust
/// use copaw_core::tool::{Tool, Toolkit, ToolError};
/// # use serde_json::json;
/// # struct DummyTool;
/// # #[async_trait::async_trait]
/// # impl Tool for DummyTool {
/// #     fn name(&self) -> &str { "dummy" }
/// #     fn description(&self) -> &str { "A dummy tool" }
/// #     fn parameters_schema(&self) -> serde_json::Value { json!({}) }
/// #     async fn execute(&self, _params: serde_json::Value) -> Result<String, ToolError> {
/// #         Ok("executed".to_string())
/// #     }
/// # }
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut toolkit = Toolkit::new();
/// let tool = DummyTool;
/// toolkit.register(Box::new(tool))?;
///
/// assert!(toolkit.get("dummy").is_some());
/// assert_eq!(toolkit.all().len(), 1);
/// # Ok(())
/// # }
/// ```
#[derive(Default)]
pub struct Toolkit {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl std::fmt::Debug for Toolkit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Toolkit")
            .field("tool_names", &self.all())
            .field("count", &self.len())
            .finish()
    }
}

impl Toolkit {
    /// Creates a new empty toolkit.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Registers a tool in the toolkit.
    ///
    /// # Parameters
    ///
    /// - `tool`: A boxed trait object implementing the `Tool` trait.
    ///
    /// # Returns
    ///
    /// - `Ok(())` if the tool was registered successfully.
    /// - `Err(ToolError::RegistrationFailed)` if a tool with the same name
    ///   already exists.
    ///
    /// # Python Reference
    ///
    /// Similar to how AgentScope tools are added to a Toolkit.
    pub fn register(&mut self, tool: Box<dyn Tool>) -> Result<(), ToolError> {
        let name = tool.name().to_string();
        if self.tools.contains_key(&name) {
            return Err(ToolError::RegistrationFailed(format!(
                "Tool '{}' already registered",
                name
            )));
        }
        self.tools.insert(name, tool);
        Ok(())
    }

    /// Gets a tool by name.
    ///
    /// # Parameters
    ///
    /// - `name`: The name of the tool to retrieve.
    ///
    /// # Returns
    ///
    /// - `Some(&dyn Tool)` if the tool exists.
    /// - `None` if the tool doesn't exist.
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    /// Returns all registered tool names.
    ///
    /// # Returns
    ///
    /// A vector of tool names in alphabetical order.
    pub fn all(&self) -> Vec<String> {
        let mut names: Vec<String> = self.tools.keys().cloned().collect();
        names.sort();
        names
    }

    /// Returns the number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Returns `true` if no tools are registered.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Unregisters a tool by name.
    ///
    /// # Parameters
    ///
    /// - `name`: The name of the tool to unregister.
    ///
    /// # Returns
    ///
    /// - `Ok(Box<dyn Tool>)` if the tool was removed.
    /// - `Err(ToolError::NotFound)` if the tool doesn't exist.
    pub fn unregister(&mut self, name: &str) -> Result<Box<dyn Tool>, ToolError> {
        self.tools
            .remove(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))
    }
}

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
            "A dummy tool for testing"
        }

        fn parameters_schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Input parameter"
                    }
                },
                "required": ["input"]
            })
        }

        async fn execute(&self, params: Value) -> Result<String, ToolError> {
            params
                .get("input")
                .and_then(|v| v.as_str())
                .map(|s| Ok(format!("executed with: {}", s)))
                .unwrap_or_else(|| {
                    Err(ToolError::InvalidParameters(
                        "Missing 'input' parameter".to_string(),
                    ))
                })
        }
    }

    struct ErrorTool;

    #[async_trait::async_trait]
    impl Tool for ErrorTool {
        fn name(&self) -> &str {
            "error_tool"
        }

        fn description(&self) -> &str {
            "A tool that always fails"
        }

        fn parameters_schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {}
            })
        }

        async fn execute(&self, _params: Value) -> Result<String, ToolError> {
            Err(ToolError::ExecutionFailed(
                "Intentional failure".to_string(),
            ))
        }
    }

    #[tokio::test]
    async fn test_tool_execute_success() {
        let tool = DummyTool;
        let result = tool
            .execute(serde_json::json!({"input": "test"}))
            .await
            .unwrap();
        assert_eq!(result, "executed with: test");
    }

    #[tokio::test]
    async fn test_tool_execute_missing_params() {
        let tool = DummyTool;
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
        match result {
            Err(ToolError::InvalidParameters(msg)) => {
                assert!(msg.contains("input"));
            }
            _ => panic!("Expected InvalidParameters error"),
        }
    }

    #[tokio::test]
    async fn test_tool_execute_error() {
        let tool = ErrorTool;
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
        match result {
            Err(ToolError::ExecutionFailed(msg)) => {
                assert_eq!(msg, "Intentional failure");
            }
            _ => panic!("Expected ExecutionFailed error"),
        }
    }

    #[test]
    fn test_toolkit_register_and_get() {
        let mut toolkit = Toolkit::new();
        assert!(toolkit.is_empty());

        toolkit.register(Box::new(DummyTool)).unwrap();
        assert!(!toolkit.is_empty());
        assert_eq!(toolkit.len(), 1);

        let tool = toolkit.get("dummy");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().name(), "dummy");
    }

    #[test]
    fn test_toolkit_duplicate_registration() {
        let mut toolkit = Toolkit::new();
        toolkit.register(Box::new(DummyTool)).unwrap();

        let result = toolkit.register(Box::new(DummyTool));
        assert!(result.is_err());
        match result {
            Err(ToolError::RegistrationFailed(msg)) => {
                assert!(msg.contains("dummy"));
                assert!(msg.contains("already registered"));
            }
            _ => panic!("Expected RegistrationFailed error"),
        }
    }

    #[test]
    fn test_toolkit_get_nonexistent() {
        let toolkit = Toolkit::new();
        assert!(toolkit.get("nonexistent").is_none());
    }

    #[test]
    fn test_toolkit_all() {
        let mut toolkit = Toolkit::new();
        toolkit.register(Box::new(DummyTool)).unwrap();
        toolkit.register(Box::new(ErrorTool)).unwrap();

        let all = toolkit.all();
        assert_eq!(all.len(), 2);
        assert!(all.contains(&"dummy".to_string()));
        assert!(all.contains(&"error_tool".to_string()));
        // Should be sorted
        assert_eq!(all, vec!["dummy".to_string(), "error_tool".to_string()]);
    }

    #[test]
    fn test_toolkit_unregister() {
        let mut toolkit = Toolkit::new();
        toolkit.register(Box::new(DummyTool)).unwrap();

        let result = toolkit.unregister("dummy");
        assert!(result.is_ok());
        assert!(toolkit.is_empty());

        let result = toolkit.unregister("dummy");
        assert!(result.is_err());
        match result {
            Err(ToolError::NotFound(name)) => {
                assert_eq!(name, "dummy");
            }
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn test_toolkit_default() {
        let toolkit = Toolkit::default();
        assert!(toolkit.is_empty());
        assert_eq!(toolkit.len(), 0);
    }

    #[test]
    fn test_tool_error_display() {
        let err = ToolError::InvalidParameters("test error".to_string());
        assert!(err.to_string().contains("Invalid parameters"));
        assert!(err.to_string().contains("test error"));

        let err = ToolError::ExecutionFailed("exec error".to_string());
        assert!(err.to_string().contains("Execution failed"));

        let err = ToolError::NotFound("my_tool".to_string());
        assert!(err.to_string().contains("Tool not found"));

        let err = ToolError::RegistrationFailed("reg error".to_string());
        assert!(err.to_string().contains("Registration failed"));
    }

    #[test]
    fn test_tool_metadata() {
        let tool = DummyTool;
        assert_eq!(tool.name(), "dummy");
        assert_eq!(tool.description(), "A dummy tool for testing");

        let schema = tool.parameters_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"].is_object());
        assert!(schema["properties"]["input"].is_object());
    }
}
