//! CoPaw Agents Crate
//!
//! This crate provides agent implementations for the CoPaw system, including
//! the ReAct-style agent with reasoning, tool use, and memory capabilities.
//!
//! # Python Reference
//!
//! - `src/copaw/agents/react_agent.py` - The main CoPawAgent
//! - `src/copaw/agents/prompt.py` - Prompt building utilities
//! - AgentScope's ReActAgent

pub mod agent;
pub mod file_manager;
pub mod react;

pub use agent::{Agent, AgentError};
pub use file_manager::{AgentFileManager, AgentFileManagerError};
pub use react::{
    BoxedPostReasoningHook, BoxedPreReasoningHook, HookContext, HookError, MockLLMProvider,
    MockTool, PostReasoningHook, PreReasoningHook, ReActAgent,
};
