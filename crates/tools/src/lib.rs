//! Built-in tools for CoPaw agent.
//!
//! This crate provides the core tool implementations for the CoPaw agent,
//! including shell command execution, file operations, and more.
//!
//! # Python Reference
//!
//! - `src/copaw/agents/tools/shell.py` - Shell command execution
//! - `src/copaw/agents/tools/file_io.py` - File operations
//! - `src/copaw/agents/tools/browser_snapshot.py` - Browser snapshot utilities
//! - `src/copaw/agents/tools/desktop_screenshot.py` - Desktop screenshot

use copaw_core::tool::{Tool, ToolError};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::io::AsyncWriteExt;

/// Error type for tools-specific operations.
#[derive(Debug, Error)]
pub enum ToolsError {
    /// Shell command execution failed.
    #[error("Shell command failed: {0}")]
    ShellError(String),

    /// Command timeout.
    #[error("Command timeout after {0} seconds")]
    Timeout(u64),

    /// Invalid file path.
    #[error("Invalid file path: {0}")]
    InvalidPath(String),

    /// File not found.
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    /// Path is outside allowed base directory.
    #[error("Path outside base directory: {0}")]
    OutsideBaseDirectory(PathBuf),

    /// Invalid line range specified.
    #[error("Invalid line range: {0}")]
    InvalidLineRange(String),

    /// Text not found for replacement.
    #[error("Text to replace not found in file")]
    TextNotFound,
}

impl From<ToolsError> for ToolError {
    fn from(err: ToolsError) -> Self {
        ToolError::ExecutionFailed(err.to_string())
    }
}

/// Shell command execution tool.
///
/// Executes shell commands with timeout and working directory support.
/// Security features include configurable working directory and timeout limits.
///
/// # Security Note
///
/// Command whitelisting is **not implemented** in this version. The tool will execute
/// any shell command provided. In production environments, consider implementing a
/// command whitelist or using a sandboxed execution environment for security.
///
/// # Python Reference
///
/// Corresponds to `execute_shell_command` in `src/copaw/agents/tools/shell.py`.
#[derive(Debug, Clone)]
pub struct ShellTool {
    /// Default working directory for command execution.
    working_dir: PathBuf,
}

impl ShellTool {
    /// Creates a new shell tool with the specified working directory.
    ///
    /// # Arguments
    ///
    /// - `working_dir`: Directory where commands will be executed.
    pub fn new<P: AsRef<Path>>(working_dir: P) -> Self {
        Self {
            working_dir: working_dir.as_ref().to_path_buf(),
        }
    }

    /// Creates a new shell tool that uses the current directory.
    pub fn current_dir() -> Self {
        Self::new(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }
}

#[async_trait::async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "execute_shell_command"
    }

    fn description(&self) -> &str {
        "Execute a shell command and return the output. Supports timeout and working directory specification."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Maximum execution time in seconds (default: 60)",
                    "default": 60
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory for command execution (optional)"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, params: Value) -> Result<String, ToolError> {
        let command = params
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidParameters("Missing 'command' parameter".to_string())
            })?;

        let timeout = params.get("timeout").and_then(|v| v.as_u64()).unwrap_or(60);

        let cwd = if let Some(cwd_str) = params.get("cwd").and_then(|v| v.as_str()) {
            PathBuf::from(cwd_str)
        } else {
            self.working_dir.clone()
        };

        self.execute_command(command, timeout, &cwd).await
    }
}

impl ShellTool {
    async fn execute_command(
        &self,
        command: &str,
        timeout_secs: u64,
        cwd: &Path,
    ) -> Result<String, ToolError> {
        use tokio::process::Command;

        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(cwd)
            .output();

        let result = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), output)
            .await
            .map_err(|_| ToolsError::Timeout(timeout_secs))??;

        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        let stdout = stdout.trim().to_string();
        let stderr = stderr.trim().to_string();

        if result.status.success() {
            if stdout.is_empty() {
                Ok("Command executed successfully (no output).".to_string())
            } else {
                Ok(stdout)
            }
        } else {
            let code = result.status.code().unwrap_or(-1);
            let mut response = format!("Command failed with exit code {}.", code);
            if !stdout.is_empty() {
                response.push_str(&format!("\n[stdout]\n{}", stdout));
            }
            if !stderr.is_empty() {
                response.push_str(&format!("\n[stderr]\n{}", stderr));
            }
            Ok(response)
        }
    }
}

/// File operations tool.
///
/// Provides read, write, edit, and append operations on files.
/// All operations are restricted to a base directory for security.
///
/// # Python Reference
///
/// Corresponds to file operations in `src/copaw/agents/tools/file_io.py`.
#[derive(Debug, Clone)]
pub struct FileTool {
    /// Base directory - operations are restricted to this directory and subdirectories.
    base_dir: PathBuf,
}

impl FileTool {
    /// Creates a new file tool with the specified base directory.
    ///
    /// # Arguments
    ///
    /// - `base_dir`: Base directory restricting file operations.
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
        }
    }

    /// Creates a new file tool with no directory restrictions.
    pub fn unrestricted() -> Self {
        Self {
            base_dir: PathBuf::from("/"),
        }
    }

    /// Resolves a file path relative to the base directory.
    ///
    /// Prevents path traversal attacks (e.g., `../`) by ensuring the resolved
    /// path is within the base directory.
    fn resolve_path(&self, file_path: &str) -> Result<PathBuf, ToolsError> {
        let path = PathBuf::from(file_path);

        // Resolve the full absolute path
        let resolved = if path.is_absolute() {
            path
        } else {
            self.base_dir.join(path)
        };

        // Normalize the path by resolving '..' and '.' components
        // std::path::absolute() doesn't normalize on all platforms, so we do it manually
        let absolute = std::path::absolute(&resolved).map_err(|e| {
            ToolsError::InvalidPath(format!("Cannot resolve path '{}': {}", file_path, e))
        })?;

        // Normalize by removing '..' and '.' components
        let normalized = Self::normalize_path(&absolute);

        // Get absolute path of base directory
        let base_absolute = std::path::absolute(&self.base_dir).map_err(|e| {
            ToolsError::InvalidPath(format!("Cannot resolve base directory: {}", e))
        })?;

        // Verify the normalized path is within the base directory
        if !normalized.starts_with(&base_absolute) {
            return Err(ToolsError::OutsideBaseDirectory(normalized));
        }

        Ok(normalized)
    }

    /// Normalizes a path by resolving '..' and '.' components.
    fn normalize_path(path: &Path) -> PathBuf {
        use std::path::Component;

        let mut result = PathBuf::new();
        let mut components = Vec::new();

        // Collect all components
        for component in path.components() {
            match component {
                Component::ParentDir => {
                    if !components.is_empty()
                        && !matches!(
                            components.last(),
                            Some(Component::Prefix(_)) | Some(Component::RootDir)
                        )
                    {
                        components.pop();
                    }
                }
                Component::CurDir => {
                    // Skip '.' components
                }
                Component::Normal(c) => {
                    components.push(Component::Normal(c));
                }
                _ => {
                    // Preserve root prefix and prefix components
                    components.push(component);
                }
            }
        }

        // Reconstruct the path
        for component in components {
            result.push(component.as_os_str());
        }

        result
    }
}

#[async_trait::async_trait]
impl Tool for FileTool {
    fn name(&self) -> &str {
        "file_operations"
    }

    fn description(&self) -> &str {
        "Perform file operations: read, write, edit, or append to files. Supports line ranges for reading."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["read", "write", "edit", "append"],
                    "description": "Type of operation to perform"
                },
                "file_path": {
                    "type": "string",
                    "description": "Path to the file"
                },
                "start_line": {
                    "type": "integer",
                    "description": "Starting line number (1-based, for read operation)"
                },
                "end_line": {
                    "type": "integer",
                    "description": "Ending line number (1-based, for read operation)"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write, append, or use as replacement text"
                },
                "old_text": {
                    "type": "string",
                    "description": "Text to find and replace (for edit operation)"
                },
                "new_text": {
                    "type": "string",
                    "description": "Replacement text (for edit operation)"
                }
            },
            "required": ["operation", "file_path"]
        })
    }

    async fn execute(&self, params: Value) -> Result<String, ToolError> {
        let operation = params
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidParameters("Missing 'operation' parameter".to_string())
            })?;

        let file_path = params
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidParameters("Missing 'file_path' parameter".to_string())
            })?;

        match operation {
            "read" => {
                let start_line = params.get("start_line").and_then(|v| v.as_i64());
                let end_line = params.get("end_line").and_then(|v| v.as_i64());
                self.read_file(file_path, start_line, end_line).await
            }
            "write" => {
                let content = params
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::InvalidParameters(
                            "Missing 'content' parameter for write".to_string(),
                        )
                    })?;
                self.write_file(file_path, content).await
            }
            "edit" => {
                let old_text =
                    params
                        .get("old_text")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            ToolError::InvalidParameters(
                                "Missing 'old_text' parameter for edit".to_string(),
                            )
                        })?;
                let new_text =
                    params
                        .get("new_text")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            ToolError::InvalidParameters(
                                "Missing 'new_text' parameter for edit".to_string(),
                            )
                        })?;
                self.edit_file(file_path, old_text, new_text).await
            }
            "append" => {
                let content = params
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::InvalidParameters(
                            "Missing 'content' parameter for append".to_string(),
                        )
                    })?;
                self.append_file(file_path, content).await
            }
            _ => Err(ToolError::InvalidParameters(format!(
                "Unknown operation: {}",
                operation
            ))),
        }
    }
}

impl FileTool {
    async fn read_file(
        &self,
        file_path: &str,
        start_line: Option<i64>,
        end_line: Option<i64>,
    ) -> Result<String, ToolError> {
        use tokio::fs;

        let path = self.resolve_path(file_path)?;

        if !path.exists() {
            return Err(ToolsError::FileNotFound(path).into());
        }

        if !path.is_file() {
            return Err(ToolError::ExecutionFailed(format!(
                "The path {} is not a file.",
                path.display()
            )));
        }

        let content = fs::read_to_string(&path)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Read file failed: {}", e)))?;

        let lines: Vec<&str> = content.lines().collect();
        let total = lines.len();

        // Check if any range parameters were specified
        let range_requested = start_line.is_some() || end_line.is_some();

        if range_requested {
            let s = std::cmp::max(1, start_line.unwrap_or(1)) as usize;
            let e = std::cmp::min(total as i64, end_line.unwrap_or(total as i64)) as usize;

            if s > total {
                return Err(ToolsError::InvalidLineRange(format!(
                    "start_line {} exceeds file length ({}) in {}",
                    s, total, file_path
                ))
                .into());
            }

            if s > e {
                return Err(ToolsError::InvalidLineRange(format!(
                    "start_line ({}) is greater than end_line ({}) in {}",
                    s, e, file_path
                ))
                .into());
            }

            let selected = &lines[s - 1..e];
            let result = selected.join("\n");
            Ok(format!(
                "{}  (lines {}-{} of {})\n{}",
                path.display(),
                s,
                e,
                total,
                result
            ))
        } else {
            Ok(content)
        }
    }

    async fn write_file(&self, file_path: &str, content: &str) -> Result<String, ToolError> {
        use tokio::fs;

        let path = self.resolve_path(file_path)?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to create directory: {}", e))
            })?;
        }

        fs::write(&path, content)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Write file failed: {}", e)))?;

        Ok(format!(
            "Wrote {} bytes to {}.",
            content.len(),
            path.display()
        ))
    }

    async fn edit_file(
        &self,
        file_path: &str,
        old_text: &str,
        new_text: &str,
    ) -> Result<String, ToolError> {
        // Read the file first
        let content = self.read_file(file_path, None, None).await?;

        if !content.contains(old_text) {
            return Err(ToolsError::TextNotFound.into());
        }

        let new_content = content.replace(old_text, new_text);
        self.write_file(file_path, &new_content).await?;

        Ok(format!("Successfully replaced text in {}.", file_path))
    }

    async fn append_file(&self, file_path: &str, content: &str) -> Result<String, ToolError> {
        use tokio::fs;

        let path = self.resolve_path(file_path)?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to create directory: {}", e))
            })?;
        }

        fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&path)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to open file: {}", e)))?
            .write_all(content.as_bytes())
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write to file: {}", e)))?;

        Ok(format!(
            "Appended {} bytes to {}.",
            content.len(),
            path.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper to create a temporary directory for testing
    fn temp_dir() -> TempDir {
        TempDir::new().expect("Failed to create temp dir")
    }

    #[test]
    fn test_shell_tool_creation() {
        let tool = ShellTool::new("/tmp");
        assert_eq!(tool.name(), "execute_shell_command");
        assert!(tool.description().contains("shell command"));
    }

    #[test]
    fn test_shell_tool_current_dir() {
        let tool = ShellTool::current_dir();
        assert_eq!(tool.name(), "execute_shell_command");
    }

    #[test]
    fn test_shell_tool_parameters_schema() {
        let tool = ShellTool::new("/tmp");
        let schema = tool.parameters_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("command")));
    }

    #[tokio::test]
    async fn test_shell_tool_execute_simple() {
        let tool = ShellTool::current_dir();
        let result = tool
            .execute(json!({"command": "echo hello"}))
            .await
            .unwrap();
        assert_eq!(result, "hello");
    }

    #[tokio::test]
    async fn test_shell_tool_execute_no_output() {
        let tool = ShellTool::current_dir();
        let result = tool.execute(json!({"command": "true"})).await.unwrap();
        assert!(result.contains("successfully"));
    }

    #[tokio::test]
    async fn test_shell_tool_execute_fails() {
        let tool = ShellTool::current_dir();
        let result = tool.execute(json!({"command": "false"})).await.unwrap();
        assert!(result.contains("failed"));
    }

    #[tokio::test]
    async fn test_shell_tool_missing_command() {
        let tool = ShellTool::current_dir();
        let result = tool.execute(json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_shell_tool_timeout() {
        let tool = ShellTool::current_dir();
        let result = tool
            .execute(json!({"command": "sleep 10", "timeout": 1}))
            .await;
        // Should timeout and return an error
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_shell_tool_custom_cwd() {
        let tmp = temp_dir();
        let tool = ShellTool::new(tmp.path());

        // Create a file in the temp directory
        let test_file = tmp.path().join("test.txt");
        tokio::fs::write(&test_file, "test content").await.unwrap();

        let result = tool
            .execute(json!({
                "command": "cat test.txt",
                "cwd": tmp.path().display().to_string()
            }))
            .await
            .unwrap();
        assert!(result.contains("test content"));
    }

    #[test]
    fn test_file_tool_creation() {
        let tool = FileTool::new("/tmp");
        assert_eq!(tool.name(), "file_operations");
        assert!(tool.description().contains("file operations"));
    }

    #[test]
    fn test_file_tool_unrestricted() {
        let tool = FileTool::unrestricted();
        assert_eq!(tool.name(), "file_operations");
    }

    #[test]
    fn test_file_tool_parameters_schema() {
        let tool = FileTool::new("/tmp");
        let schema = tool.parameters_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("operation")));
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("file_path")));
    }

    #[test]
    fn test_file_tool_resolve_path() {
        let tmp = temp_dir();
        let tool = FileTool::new(tmp.path());

        // Relative path should be resolved to base dir
        let resolved = tool.resolve_path("test.txt").unwrap();
        assert!(resolved.starts_with(tmp.path()));
        assert_eq!(resolved.file_name().unwrap(), "test.txt");

        // Absolute path should remain as-is (if within base dir)
        let abs_path = tmp.path().join("abs.txt");
        let resolved = tool.resolve_path(abs_path.to_str().unwrap()).unwrap();
        assert_eq!(resolved, abs_path);
    }

    #[tokio::test]
    async fn test_file_tool_write_and_read() {
        let tmp = temp_dir();
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
        assert!(write_result.contains("Wrote 13 bytes"));

        // Read the file
        let read_result = tool
            .execute(json!({
                "operation": "read",
                "file_path": "test.txt"
            }))
            .await
            .unwrap();
        assert_eq!(read_result, "Hello, World!");
    }

    #[tokio::test]
    async fn test_file_tool_read_line_range() {
        let tmp = temp_dir();
        let tool = FileTool::new(tmp.path());

        // Write a file with multiple lines
        let content = "line1\nline2\nline3\nline4\nline5";
        tool.write_file("test.txt", content).await.unwrap();

        // Read lines 2-4
        let read_result = tool
            .execute(json!({
                "operation": "read",
                "file_path": "test.txt",
                "start_line": 2,
                "end_line": 4
            }))
            .await
            .unwrap();
        assert!(read_result.contains("line2"));
        assert!(read_result.contains("line3"));
        assert!(read_result.contains("line4"));
        assert!(read_result.contains("lines 2-4 of 5"));
        assert!(!read_result.contains("line1"));
        assert!(!read_result.contains("line5"));
    }

    #[tokio::test]
    async fn test_file_tool_read_invalid_range() {
        let tmp = temp_dir();
        let tool = FileTool::new(tmp.path());

        tool.write_file("test.txt", "line1\nline2\nline3")
            .await
            .unwrap();

        // Start line > total lines
        let result = tool
            .execute(json!({
                "operation": "read",
                "file_path": "test.txt",
                "start_line": 10
            }))
            .await;
        assert!(result.is_err());

        // Start > end
        let result = tool
            .execute(json!({
                "operation": "read",
                "file_path": "test.txt",
                "start_line": 3,
                "end_line": 1
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_file_tool_edit() {
        let tmp = temp_dir();
        let tool = FileTool::new(tmp.path());

        // Write initial content
        tool.write_file("test.txt", "Hello, World!").await.unwrap();

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
        assert!(edit_result.contains("Successfully replaced"));

        // Verify the change
        let read_result = tool.read_file("test.txt", None, None).await.unwrap();
        assert_eq!(read_result, "Hello, Rust!");
    }

    #[tokio::test]
    async fn test_file_tool_edit_text_not_found() {
        let tmp = temp_dir();
        let tool = FileTool::new(tmp.path());

        tool.write_file("test.txt", "Hello, World!").await.unwrap();

        let result = tool
            .execute(json!({
                "operation": "edit",
                "file_path": "test.txt",
                "old_text": "Python",
                "new_text": "Rust"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_file_tool_append() {
        let tmp = temp_dir();
        let tool = FileTool::new(tmp.path());

        // Write initial content
        tool.write_file("test.txt", "Hello").await.unwrap();

        // Append to the file
        let append_result = tool
            .execute(json!({
                "operation": "append",
                "file_path": "test.txt",
                "content": ", World!"
            }))
            .await
            .unwrap();
        assert!(append_result.contains("Appended"));

        // Verify the content
        let read_result = tool.read_file("test.txt", None, None).await.unwrap();
        assert_eq!(read_result, "Hello, World!");
    }

    #[tokio::test]
    async fn test_file_tool_file_not_found() {
        let tmp = temp_dir();
        let tool = FileTool::new(tmp.path());

        let result = tool
            .execute(json!({
                "operation": "read",
                "file_path": "nonexistent.txt"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_file_tool_missing_operation() {
        let tmp = temp_dir();
        let tool = FileTool::new(tmp.path());

        let result = tool.execute(json!({"file_path": "test.txt"})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_file_tool_invalid_operation() {
        let tmp = temp_dir();
        let tool = FileTool::new(tmp.path());

        let result = tool
            .execute(json!({
                "operation": "delete",
                "file_path": "test.txt"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_file_tool_missing_content_for_write() {
        let tmp = temp_dir();
        let tool = FileTool::new(tmp.path());

        let result = tool
            .execute(json!({
                "operation": "write",
                "file_path": "test.txt"
            }))
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_tools_error_display() {
        let err = ToolsError::ShellError("command failed".to_string());
        assert!(err.to_string().contains("Shell command failed"));

        let err = ToolsError::Timeout(30);
        assert!(err.to_string().contains("30 seconds"));

        let err = ToolsError::InvalidPath("bad/path".to_string());
        assert!(err.to_string().contains("Invalid file path"));

        let err = ToolsError::FileNotFound(PathBuf::from("/missing.txt"));
        assert!(err.to_string().contains("File not found"));

        let err = ToolsError::InvalidLineRange("bad range".to_string());
        assert!(err.to_string().contains("Invalid line range"));

        let err = ToolsError::TextNotFound;
        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_file_tool_nested_directory() {
        let tmp = temp_dir();
        let tool = FileTool::new(tmp.path());

        // Write to a nested path (should create directories)
        let write_result = tool
            .execute(json!({
                "operation": "write",
                "file_path": "nested/dir/test.txt",
                "content": "nested content"
            }))
            .await
            .unwrap();
        assert!(write_result.contains("Wrote"));

        // Verify the file was created
        let read_result = tool
            .execute(json!({
                "operation": "read",
                "file_path": "nested/dir/test.txt"
            }))
            .await
            .unwrap();
        assert_eq!(read_result, "nested content");
    }

    #[tokio::test]
    async fn test_file_tool_path_traversal_protection() {
        let tmp = temp_dir();
        let tool = FileTool::new(tmp.path());

        // Create a file inside temp directory
        tool.write_file("inside.txt", "safe content").await.unwrap();

        // Create a nested directory structure
        let nested_dir = tmp.path().join("safe_dir");
        tokio::fs::create_dir_all(&nested_dir).await.unwrap();
        let nested_tool = FileTool::new(&nested_dir);

        // Create a file in the nested directory
        nested_tool
            .write_file("nested_file.txt", "nested content")
            .await
            .unwrap();

        // Try to escape using ../../.. to access parent of base directory
        let result = nested_tool
            .read_file("../../../outside.txt", None, None)
            .await;
        assert!(result.is_err());

        // Try to read file using ../ path traversal - should be blocked
        let result = nested_tool.read_file("../../inside.txt", None, None).await;
        // This should also fail since it's outside the base_dir
        assert!(result.is_err());

        // Try to write using ../ path traversal - should be blocked
        let result = nested_tool
            .write_file("../../malicious.txt", "malicious content")
            .await;
        assert!(result.is_err());

        // Verify the internal file in nested directory is still accessible
        let result = nested_tool.read_file("nested_file.txt", None, None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "nested content");

        // Test absolute path escaping - try to access /tmp or root
        let result = nested_tool.read_file("/tmp", None, None).await;
        assert!(result.is_err());

        let result = nested_tool.read_file("/etc/passwd", None, None).await;
        assert!(result.is_err());
    }
}
