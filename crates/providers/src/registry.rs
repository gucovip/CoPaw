// -*- coding: utf-8 -*-
// Built-in provider definitions and registry.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Model identifier and display name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelInfo {
    /// Model identifier used in API calls.
    pub id: String,
    /// Human-readable model name.
    pub name: String,
}

impl ModelInfo {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

/// Static definition of a provider (built-in or custom).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderDefinition {
    /// Provider identifier.
    pub id: String,
    /// Human-readable provider name.
    pub name: String,
    /// Default API base URL.
    #[serde(default)]
    pub default_base_url: String,
    /// Expected prefix for the API key.
    #[serde(default)]
    pub api_key_prefix: String,
    /// Built-in LLM model list.
    #[serde(default)]
    pub models: Vec<ModelInfo>,
    /// Whether this is a custom provider.
    #[serde(default)]
    pub is_custom: bool,
    /// Whether this is a local provider.
    #[serde(default)]
    pub is_local: bool,
    /// Chat model class name (e.g., "OpenAIChatModel").
    #[serde(default = "default_chat_model")]
    pub chat_model: String,
}

fn default_chat_model() -> String {
    "OpenAIChatModel".to_string()
}

impl ProviderDefinition {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            default_base_url: String::new(),
            api_key_prefix: String::new(),
            models: Vec::new(),
            is_custom: false,
            is_local: false,
            chat_model: default_chat_model(),
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.default_base_url = base_url.into();
        self
    }

    pub fn with_api_key_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.api_key_prefix = prefix.into();
        self
    }

    pub fn with_models(mut self, models: Vec<ModelInfo>) -> Self {
        self.models = models;
        self
    }

    pub fn with_local(mut self, is_local: bool) -> Self {
        self.is_local = is_local;
        self
    }
}

// Built-in provider models
fn modelscope_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "Qwen/Qwen3-235B-A22B-Instruct-2507".to_string(),
            name: "Qwen3-235B-A22B-Instruct-2507".to_string(),
        },
        ModelInfo {
            id: "deepseek-ai/DeepSeek-V3.2".to_string(),
            name: "DeepSeek-V3.2".to_string(),
        },
    ]
}

fn dashscope_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "qwen3-max".to_string(),
            name: "Qwen3 Max".to_string(),
        },
        ModelInfo {
            id: "qwen3-235b-a22b-thinking-2507".to_string(),
            name: "Qwen3 235B A22B Thinking".to_string(),
        },
        ModelInfo {
            id: "deepseek-v3.2".to_string(),
            name: "DeepSeek-V3.2".to_string(),
        },
    ]
}

fn aliyun_codingplan_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "qwen3.5-plus".to_string(),
            name: "Qwen3.5 Plus".to_string(),
        },
        ModelInfo {
            id: "glm-5".to_string(),
            name: "GLM-5".to_string(),
        },
        ModelInfo {
            id: "glm-4.7".to_string(),
            name: "GLM-4.7".to_string(),
        },
        ModelInfo {
            id: "MiniMax-M2.5".to_string(),
            name: "MiniMax M2.5".to_string(),
        },
        ModelInfo {
            id: "kimi-k2.5".to_string(),
            name: "Kimi K2.5".to_string(),
        },
        ModelInfo {
            id: "qwen3-max-2026-01-23".to_string(),
            name: "Qwen3 Max 2026-01-23".to_string(),
        },
        ModelInfo {
            id: "qwen3-coder-next".to_string(),
            name: "Qwen3 Coder Next".to_string(),
        },
        ModelInfo {
            id: "qwen3-coder-plus".to_string(),
            name: "Qwen3 Coder Plus".to_string(),
        },
    ]
}

fn openai_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "gpt-5.2".to_string(),
            name: "GPT-5.2".to_string(),
        },
        ModelInfo {
            id: "gpt-5".to_string(),
            name: "GPT-5".to_string(),
        },
        ModelInfo {
            id: "gpt-5-mini".to_string(),
            name: "GPT-5 Mini".to_string(),
        },
        ModelInfo {
            id: "gpt-5-nano".to_string(),
            name: "GPT-5 Nano".to_string(),
        },
        ModelInfo {
            id: "gpt-4.1".to_string(),
            name: "GPT-4.1".to_string(),
        },
        ModelInfo {
            id: "gpt-4.1-mini".to_string(),
            name: "GPT-4.1 Mini".to_string(),
        },
        ModelInfo {
            id: "gpt-4.1-nano".to_string(),
            name: "GPT-4.1 Nano".to_string(),
        },
        ModelInfo {
            id: "o3".to_string(),
            name: "o3".to_string(),
        },
        ModelInfo {
            id: "o4-mini".to_string(),
            name: "o4-mini".to_string(),
        },
        ModelInfo {
            id: "gpt-4o".to_string(),
            name: "GPT-4o".to_string(),
        },
        ModelInfo {
            id: "gpt-4o-mini".to_string(),
            name: "GPT-4o Mini".to_string(),
        },
    ]
}

fn azure_openai_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "gpt-5.2".to_string(),
            name: "GPT-5.2".to_string(),
        },
        ModelInfo {
            id: "gpt-5".to_string(),
            name: "GPT-5".to_string(),
        },
        ModelInfo {
            id: "gpt-5-mini".to_string(),
            name: "GPT-5 Mini".to_string(),
        },
        ModelInfo {
            id: "gpt-5-nano".to_string(),
            name: "GPT-5 Nano".to_string(),
        },
        ModelInfo {
            id: "gpt-4.1".to_string(),
            name: "GPT-4.1".to_string(),
        },
        ModelInfo {
            id: "gpt-4.1-mini".to_string(),
            name: "GPT-4.1 Mini".to_string(),
        },
        ModelInfo {
            id: "gpt-4.1-nano".to_string(),
            name: "GPT-4.1 Nano".to_string(),
        },
        ModelInfo {
            id: "gpt-4o".to_string(),
            name: "GPT-4o".to_string(),
        },
        ModelInfo {
            id: "gpt-4o-mini".to_string(),
            name: "GPT-4o Mini".to_string(),
        },
    ]
}

fn anthropic_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "claude-opus-4-1-20250805".to_string(),
            name: "Claude Opus 4.1".to_string(),
        },
        ModelInfo {
            id: "claude-sonnet-4-20250514".to_string(),
            name: "Claude Sonnet 4".to_string(),
        },
        ModelInfo {
            id: "claude-3-7-sonnet-latest".to_string(),
            name: "Claude 3.7 Sonnet".to_string(),
        },
        ModelInfo {
            id: "claude-3-5-haiku-latest".to_string(),
            name: "Claude 3.5 Haiku".to_string(),
        },
    ]
}

/// Built-in provider definitions.
pub fn builtin_providers() -> Vec<ProviderDefinition> {
    vec![
        ProviderDefinition {
            id: "modelscope".to_string(),
            name: "ModelScope".to_string(),
            default_base_url: "https://api-inference.modelscope.cn/v1".to_string(),
            api_key_prefix: "ms".to_string(),
            models: modelscope_models(),
            is_custom: false,
            is_local: false,
            chat_model: default_chat_model(),
        },
        ProviderDefinition {
            id: "dashscope".to_string(),
            name: "DashScope".to_string(),
            default_base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
            api_key_prefix: "sk".to_string(),
            models: dashscope_models(),
            is_custom: false,
            is_local: false,
            chat_model: default_chat_model(),
        },
        ProviderDefinition {
            id: "aliyun-codingplan".to_string(),
            name: "Aliyun Coding Plan".to_string(),
            default_base_url: "https://coding.dashscope.aliyuncs.com/v1".to_string(),
            api_key_prefix: "sk-sp".to_string(),
            models: aliyun_codingplan_models(),
            is_custom: false,
            is_local: false,
            chat_model: default_chat_model(),
        },
        ProviderDefinition {
            id: "llamacpp".to_string(),
            name: "llama.cpp (Local)".to_string(),
            default_base_url: String::new(),
            api_key_prefix: String::new(),
            models: Vec::new(),
            is_custom: false,
            is_local: true,
            chat_model: default_chat_model(),
        },
        ProviderDefinition {
            id: "mlx".to_string(),
            name: "MLX (Local, Apple Silicon)".to_string(),
            default_base_url: String::new(),
            api_key_prefix: String::new(),
            models: Vec::new(),
            is_custom: false,
            is_local: true,
            chat_model: default_chat_model(),
        },
        ProviderDefinition {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            default_base_url: "https://api.openai.com/v1".to_string(),
            api_key_prefix: "sk-".to_string(),
            models: openai_models(),
            is_custom: false,
            is_local: false,
            chat_model: default_chat_model(),
        },
        ProviderDefinition {
            id: "azure-openai".to_string(),
            name: "Azure OpenAI".to_string(),
            default_base_url: String::new(),
            api_key_prefix: String::new(),
            models: azure_openai_models(),
            is_custom: false,
            is_local: false,
            chat_model: default_chat_model(),
        },
        ProviderDefinition {
            id: "anthropic".to_string(),
            name: "Anthropic".to_string(),
            default_base_url: "https://api.anthropic.com/v1".to_string(),
            api_key_prefix: "sk-ant-".to_string(),
            models: anthropic_models(),
            is_custom: false,
            is_local: false,
            chat_model: default_chat_model(),
        },
        ProviderDefinition {
            id: "ollama".to_string(),
            name: "Ollama".to_string(),
            default_base_url: "http://localhost:11434/v1".to_string(),
            api_key_prefix: String::new(),
            models: Vec::new(),
            is_custom: false,
            is_local: false,
            chat_model: default_chat_model(),
        },
    ]
}

/// Built-in provider IDs.
pub const BUILTIN_PROVIDER_IDS: &[&str] = &[
    "modelscope",
    "dashscope",
    "aliyun-codingplan",
    "openai",
    "azure-openai",
    "anthropic",
    "ollama",
    "llamacpp",
    "mlx",
];

/// Thread-safe provider registry.
#[derive(Debug, Clone)]
pub struct ProviderRegistry {
    providers: Arc<RwLock<HashMap<String, ProviderDefinition>>>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        let mut providers = HashMap::new();
        for provider in builtin_providers() {
            providers.insert(provider.id.clone(), provider);
        }
        Self {
            providers: Arc::new(RwLock::new(providers)),
        }
    }
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a provider by ID.
    pub fn get(&self, provider_id: &str) -> Option<ProviderDefinition> {
        self.providers
            .read()
            .ok()
            .and_then(|p| p.get(provider_id).cloned())
    }

    /// List all providers.
    pub fn list(&self) -> Vec<ProviderDefinition> {
        self.providers
            .read()
            .map(|p| p.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Check if a provider ID is built-in.
    pub fn is_builtin(&self, provider_id: &str) -> bool {
        BUILTIN_PROVIDER_IDS.contains(&provider_id)
    }

    /// Validate a custom provider ID.
    /// Returns an error message if invalid, or None if valid.
    pub fn validate_custom_id(&self, provider_id: &str) -> Option<String> {
        if self.is_builtin(provider_id) {
            return Some(format!(
                "'{provider_id}' is a built-in provider id and cannot be used."
            ));
        }
        // Must start with a lowercase letter and contain only lowercase letters,
        // digits, hyphens, and underscores (max 64 chars).
        let valid_id_re = regex::Regex::new(r"^[a-z][a-z0-9_-]{0,63}$").unwrap();
        if !valid_id_re.is_match(provider_id) {
            return Some(format!(
                "Invalid provider id '{provider_id}'. Must start with a lowercase letter and contain only lowercase letters, digits, hyphens, and underscores (max 64 chars)."
            ));
        }
        None
    }

    /// Register a custom provider.
    pub fn register_custom(&self, provider: ProviderDefinition) -> Result<(), String> {
        if let Some(err) = self.validate_custom_id(&provider.id) {
            return Err(err);
        }
        let mut providers = self.providers.write().map_err(|e| e.to_string())?;
        // Check if provider already exists
        if providers.contains_key(&provider.id) {
            return Err(format!("Provider '{}' already exists.", provider.id));
        }
        providers.insert(provider.id.clone(), provider);
        Ok(())
    }

    /// Unregister a custom provider.
    pub fn unregister_custom(&self, provider_id: &str) -> Result<(), String> {
        if self.is_builtin(provider_id) {
            return Err(format!("Cannot remove built-in provider '{provider_id}'."));
        }
        let mut providers = self.providers.write().map_err(|e| e.to_string())?;
        providers.remove(provider_id);
        Ok(())
    }

    /// Sync custom providers from the given map.
    pub fn sync_custom(&self, custom_providers: &HashMap<String, ProviderDefinition>) {
        if let Ok(mut providers) = self.providers.write() {
            // Remove stale custom providers
            let ids_to_remove: Vec<String> = providers
                .iter()
                .filter(|(_, p)| p.is_custom && !custom_providers.contains_key(&p.id))
                .map(|(id, _)| id.clone())
                .collect();
            for id in ids_to_remove {
                providers.remove(&id);
            }
            // Add/update custom providers
            for (id, provider) in custom_providers {
                providers.insert(id.clone(), provider.clone());
            }
        }
    }

    /// Update models for a local provider.
    pub fn update_local_models(&self, provider_id: &str, models: Vec<ModelInfo>) {
        if let Ok(mut providers) = self.providers.write() {
            if let Some(provider) = providers.get_mut(provider_id) {
                provider.models = models;
            }
        }
    }

    /// Get the chat model class name for a provider.
    pub fn get_chat_model(&self, provider_id: &str) -> String {
        self.get(provider_id)
            .map(|p| p.chat_model)
            .unwrap_or_else(|| "OpenAIChatModel".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_providers() {
        let providers = builtin_providers();
        assert_eq!(providers.len(), 9);
        assert!(providers.iter().any(|p| p.id == "openai"));
        assert!(providers.iter().any(|p| p.id == "anthropic"));
        assert!(providers.iter().any(|p| p.id == "ollama"));
    }

    #[test]
    fn test_registry_get() {
        let registry = ProviderRegistry::new();
        let openai = registry.get("openai");
        assert!(openai.is_some());
        assert_eq!(openai.unwrap().name, "OpenAI");

        let nonexistent = registry.get("nonexistent");
        assert!(nonexistent.is_none());
    }

    #[test]
    fn test_registry_list() {
        let registry = ProviderRegistry::new();
        let providers = registry.list();
        assert_eq!(providers.len(), 9);
    }

    #[test]
    fn test_is_builtin() {
        let registry = ProviderRegistry::new();
        assert!(registry.is_builtin("openai"));
        assert!(registry.is_builtin("anthropic"));
        assert!(registry.is_builtin("ollama"));
        assert!(!registry.is_builtin("custom"));
    }

    #[test]
    fn test_validate_custom_id() {
        let registry = ProviderRegistry::new();
        assert!(registry.validate_custom_id("custom").is_none());
        assert!(registry.validate_custom_id("my-provider-123").is_none());
        assert!(registry.validate_custom_id("openai").is_some()); // built-in
        assert!(registry.validate_custom_id("123invalid").is_some()); // starts with digit
        assert!(registry.validate_custom_id("Invalid").is_some()); // uppercase
        assert!(registry
            .validate_custom_id("a".repeat(65).as_str())
            .is_some()); // too long
    }

    #[test]
    fn test_register_custom() {
        let registry = ProviderRegistry::new();
        let custom = ProviderDefinition {
            id: "my-provider".to_string(),
            name: "My Provider".to_string(),
            is_custom: true,
            ..Default::default()
        };

        assert!(registry.register_custom(custom.clone()).is_ok());
        assert!(registry.get("my-provider").is_some());

        // Duplicate should fail
        assert!(registry.register_custom(custom).is_err());
    }

    #[test]
    fn test_unregister_custom() {
        let registry = ProviderRegistry::new();
        let custom = ProviderDefinition {
            id: "my-provider".to_string(),
            name: "My Provider".to_string(),
            is_custom: true,
            ..Default::default()
        };

        registry.register_custom(custom).unwrap();
        assert!(registry.get("my-provider").is_some());

        assert!(registry.unregister_custom("my-provider").is_ok());
        assert!(registry.get("my-provider").is_none());

        // Cannot unregister built-in
        assert!(registry.unregister_custom("openai").is_err());
    }
}
