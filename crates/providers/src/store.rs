// -*- coding: utf-8 -*-
// Reading and writing provider configuration (providers.json).

use crate::registry::{ModelInfo, ProviderDefinition, ProviderRegistry, BUILTIN_PROVIDER_IDS};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Per-provider settings stored in providers.json (built-in only).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderSettings {
    /// Configured base URL (may override default).
    #[serde(default)]
    pub base_url: String,
    /// API key.
    #[serde(default)]
    pub api_key: String,
    /// Extra models added by user.
    #[serde(default)]
    pub extra_models: Vec<ModelInfo>,
    /// Chat model class name override.
    #[serde(default)]
    pub chat_model: String,
}

/// Persisted definition + runtime config of a user-created custom provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProviderData {
    /// Provider identifier (unique).
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
    /// Configured base URL override.
    #[serde(default)]
    pub base_url: String,
    /// API key.
    #[serde(default)]
    pub api_key: String,
    /// Chat model class name.
    #[serde(default = "default_chat_model")]
    pub chat_model: String,
}

fn default_chat_model() -> String {
    "OpenAIChatModel".to_string()
}

impl CustomProviderData {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            default_base_url: String::new(),
            api_key_prefix: String::new(),
            models: Vec::new(),
            base_url: String::new(),
            api_key: String::new(),
            chat_model: default_chat_model(),
        }
    }

    /// Get effective base URL (configured override or default).
    pub fn effective_base_url(&self) -> &str {
        if self.base_url.is_empty() {
            &self.default_base_url
        } else {
            &self.base_url
        }
    }

    /// Convert to ProviderDefinition.
    pub fn to_definition(&self) -> ProviderDefinition {
        ProviderDefinition {
            id: self.id.clone(),
            name: self.name.clone(),
            default_base_url: self.default_base_url.clone(),
            api_key_prefix: self.api_key_prefix.clone(),
            models: self.models.clone(),
            is_custom: true,
            is_local: false,
            chat_model: self.chat_model.clone(),
        }
    }
}

/// Active model slot configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelSlotConfig {
    /// Provider ID.
    #[serde(default)]
    pub provider_id: String,
    /// Model identifier.
    #[serde(default)]
    pub model: String,
}

impl ModelSlotConfig {
    pub fn new(provider_id: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            model: model.into(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.provider_id.is_empty() && self.model.is_empty()
    }
}

/// Top-level structure of providers.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvidersData {
    /// Per-provider settings for built-in providers.
    #[serde(default)]
    pub providers: HashMap<String, ProviderSettings>,
    /// Custom provider definitions.
    #[serde(default)]
    pub custom_providers: HashMap<String, CustomProviderData>,
    /// Active LLM configuration.
    #[serde(default)]
    pub active_llm: ModelSlotConfig,
}

impl Default for ProvidersData {
    fn default() -> Self {
        Self {
            providers: HashMap::new(),
            custom_providers: HashMap::new(),
            active_llm: ModelSlotConfig::default(),
        }
    }
}

impl ProvidersData {
    /// Get credentials for a provider.
    pub fn get_credentials(&self, provider_id: &str) -> (String, String) {
        if let Some(cpd) = self.custom_providers.get(provider_id) {
            return (cpd.effective_base_url().to_string(), cpd.api_key.clone());
        }
        if let Some(settings) = self.providers.get(provider_id) {
            return (settings.base_url.clone(), settings.api_key.clone());
        }
        (String::new(), String::new())
    }

    /// Check if a provider is configured.
    pub fn is_configured(&self, defn: &ProviderDefinition) -> bool {
        // Local providers are always considered configured
        if defn.is_local {
            return true;
        }

        // Ollama is configured if a base_url exists in settings.
        if defn.id == "ollama" {
            return self
                .providers
                .get(&defn.id)
                .map(|s| !s.base_url.is_empty())
                .unwrap_or(false);
        }

        // Custom providers need base_url
        if let Some(cpd) = self.custom_providers.get(&defn.id) {
            return !cpd.effective_base_url().is_empty();
        }

        // Built-in remote providers are considered configured when settings exist.
        self.providers.contains_key(&defn.id)
    }
}

/// Provider store for loading and saving providers.json.
#[derive(Debug, Clone)]
pub struct ProviderStore {
    /// Path to providers.json.
    pub path: PathBuf,
    /// Provider registry.
    pub registry: Arc<ProviderRegistry>,
}

impl ProviderStore {
    fn expand_home(path: &str) -> PathBuf {
        if let Some(home) = dirs::home_dir() {
            if let Some(rest) = path.strip_prefix('~') {
                return home.join(rest.trim_start_matches('/'));
            }
        }
        PathBuf::from(path)
    }

    fn bootstrap_working_dir() -> PathBuf {
        let raw = env::var("COPAW_WORKING_DIR").unwrap_or_else(|_| "~/.copaw".to_string());
        Self::expand_home(&raw)
    }

    fn bootstrap_secret_dir() -> PathBuf {
        if let Ok(secret) = env::var("COPAW_SECRET_DIR") {
            return Self::expand_home(&secret);
        }
        let wd = Self::bootstrap_working_dir();
        PathBuf::from(format!("{}.secret", wd.display()))
    }

    fn same_path(a: &Path, b: &Path) -> bool {
        match (a.canonicalize(), b.canonicalize()) {
            (Ok(aa), Ok(bb)) => aa == bb,
            _ => false,
        }
    }

    fn legacy_candidates() -> Vec<PathBuf> {
        vec![
            PathBuf::from("src/copaw/providers/providers.json"),
            Self::bootstrap_working_dir().join("providers.json"),
        ]
    }

    fn migrate_legacy_providers_json(&self) {
        if self.path.is_file() {
            return;
        }
        if self.path.exists() && !self.path.is_file() {
            return;
        }

        for legacy in Self::legacy_candidates() {
            if !legacy.is_file() || Self::same_path(&legacy, &self.path) {
                continue;
            }
            if let Some(parent) = self.path.parent() {
                if fs::create_dir_all(parent).is_err() {
                    continue;
                }
            }
            if fs::copy(&legacy, &self.path).is_ok() {
                break;
            }
        }
    }

    pub fn new(path: impl AsRef<Path>, registry: Arc<ProviderRegistry>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            registry,
        }
    }

    /// Get the default providers.json path.
    pub fn default_path() -> PathBuf {
        Self::bootstrap_secret_dir().join("providers.json")
    }

    /// Load providers.json, creating/repairing as needed.
    pub fn load(&self) -> Result<ProvidersData, String> {
        self.migrate_legacy_providers_json();
        if self.path.exists() && !self.path.is_file() {
            return Err(format!(
                "providers.json path exists but is not a regular file: {}",
                self.path.display()
            ));
        }

        let mut data = if self.path.exists() {
            let content = std::fs::read_to_string(&self.path)
                .map_err(|e| format!("Failed to read providers.json: {e}"))?;
            serde_json::from_str(&content).unwrap_or_else(|_| ProvidersData::default())
        } else {
            ProvidersData::default()
        };

        // Sync custom providers with registry
        self.sync_custom_to_registry(&data)?;

        // Ensure all built-in providers have entries
        self.ensure_builtin_providers(&mut data);

        // Validate active_llm
        self.validate_active_llm(&mut data);

        // Save to ensure consistency
        self.save(&data)?;

        Ok(data)
    }

    /// Save providers.json.
    pub fn save(&self, data: &ProvidersData) -> Result<(), String> {
        self.migrate_legacy_providers_json();
        if self.path.exists() && !self.path.is_file() {
            return Err(format!(
                "providers.json path exists but is not a regular file: {}",
                self.path.display()
            ));
        }

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {e}"))?;
        }

        let json = serde_json::to_string_pretty(data)
            .map_err(|e| format!("Failed to serialize providers.json: {e}"))?;

        std::fs::write(&self.path, json)
            .map_err(|e| format!("Failed to write providers.json: {e}"))?;

        Ok(())
    }

    /// Sync custom providers to the registry.
    fn sync_custom_to_registry(&self, data: &ProvidersData) -> Result<(), String> {
        let mut custom_defs = HashMap::new();
        for cpd in data.custom_providers.values() {
            custom_defs.insert(cpd.id.clone(), cpd.to_definition());
        }
        self.registry.sync_custom(&custom_defs);
        Ok(())
    }

    /// Ensure all built-in providers have entries.
    fn ensure_builtin_providers(&self, data: &mut ProvidersData) {
        let providers = self.registry.list();
        for provider in providers {
            if provider.is_local {
                // Local providers don't need ProviderSettings
                data.providers.remove(&provider.id);
                continue;
            }

            // Only add if there's a default base URL (avoid adding entries for providers without defaults)
            if !provider.default_base_url.is_empty() {
                data.providers
                    .entry(provider.id.clone())
                    .or_insert_with(|| ProviderSettings {
                        base_url: provider.default_base_url.clone(),
                        ..Default::default()
                    });
            }
        }
    }

    /// Validate and clear active_llm if provider is not configured.
    fn validate_active_llm(&self, data: &mut ProvidersData) {
        let pid = data.active_llm.provider_id.clone();
        if pid.is_empty() {
            return;
        }

        if let Some(defn) = self.registry.get(&pid) {
            if !data.is_configured(&defn) {
                data.active_llm = ModelSlotConfig::default();
            }
        } else {
            data.active_llm = ModelSlotConfig::default();
        }
    }

    /// Update provider settings.
    pub fn update_settings(
        &self,
        provider_id: &str,
        api_key: Option<String>,
        base_url: Option<String>,
    ) -> Result<ProvidersData, String> {
        let mut data = self.load()?;

        if let Some(cpd) = data.custom_providers.get_mut(provider_id) {
            if let Some(ref key) = api_key {
                cpd.api_key = key.clone();
            }
            if let Some(ref url) = base_url {
                cpd.base_url = url.clone();
            }
            if cpd.base_url.is_empty() {
                cpd.base_url = cpd.default_base_url.clone();
            }
            // Update registry
            self.registry.register_custom(cpd.to_definition())?;
        } else {
            let settings = data.providers.entry(provider_id.to_string()).or_default();
            if let Some(ref key) = api_key {
                settings.api_key = key.clone();
            }
            if let Some(ref url) = base_url {
                settings.base_url = url.clone();
            }
            if settings.base_url.is_empty() {
                if let Some(defn) = self.registry.get(provider_id) {
                    settings.base_url = defn.default_base_url;
                }
            }
        }

        // Clear active_llm if api_key was cleared
        if api_key.as_ref().map_or(false, |k| k.is_empty())
            && data.active_llm.provider_id == provider_id
        {
            data.active_llm = ModelSlotConfig::default();
        }

        self.save(&data)?;
        Ok(data)
    }

    /// Set active LLM.
    pub fn set_active_llm(&self, provider_id: &str, model: &str) -> Result<ProvidersData, String> {
        let mut data = self.load()?;
        data.active_llm = ModelSlotConfig::new(provider_id, model);
        self.save(&data)?;
        Ok(data)
    }

    /// Create a custom provider.
    pub fn create_custom(
        &self,
        provider_id: &str,
        name: &str,
        default_base_url: &str,
        api_key_prefix: &str,
        models: Vec<ModelInfo>,
    ) -> Result<ProvidersData, String> {
        // Validate ID
        if let Some(err) = self.registry.validate_custom_id(provider_id) {
            return Err(err);
        }

        let mut data = self.load()?;

        if data.custom_providers.contains_key(provider_id) {
            return Err(format!("Custom provider '{provider_id}' already exists."));
        }

        let cpd = CustomProviderData {
            id: provider_id.to_string(),
            name: name.to_string(),
            default_base_url: default_base_url.to_string(),
            api_key_prefix: api_key_prefix.to_string(),
            models,
            base_url: default_base_url.to_string(),
            api_key: String::new(),
            chat_model: default_chat_model(),
        };

        data.custom_providers
            .insert(provider_id.to_string(), cpd.clone());

        // Register in registry
        self.registry.register_custom(cpd.to_definition())?;

        self.save(&data)?;
        Ok(data)
    }

    /// Delete a custom provider.
    pub fn delete_custom(&self, provider_id: &str) -> Result<ProvidersData, String> {
        if BUILTIN_PROVIDER_IDS.contains(&provider_id) {
            return Err(format!("Cannot delete built-in provider '{provider_id}'."));
        }

        let mut data = self.load()?;

        if !data.custom_providers.contains_key(provider_id) {
            return Err(format!("Custom provider '{provider_id}' not found."));
        }

        data.custom_providers.remove(provider_id);
        self.registry.unregister_custom(provider_id)?;

        if data.active_llm.provider_id == provider_id {
            data.active_llm = ModelSlotConfig::default();
        }

        self.save(&data)?;
        Ok(data)
    }

    /// Add a model to a provider.
    pub fn add_model(&self, provider_id: &str, model: ModelInfo) -> Result<ProvidersData, String> {
        let defn = self
            .registry
            .get(provider_id)
            .ok_or_else(|| format!("Provider '{provider_id}' not found."))?;

        let mut data = self.load()?;

        if defn.id == "ollama" {
            return Err("Cannot add models to built-in provider 'ollama'. Ollama models are managed by the Ollama daemon itself.".to_string());
        }

        if BUILTIN_PROVIDER_IDS.contains(&provider_id) {
            let settings = data.providers.entry(provider_id.to_string()).or_default();
            let all_ids: std::collections::HashSet<_> = defn
                .models
                .iter()
                .map(|m| &m.id)
                .chain(settings.extra_models.iter().map(|m| &m.id))
                .collect();

            if all_ids.contains(&model.id) {
                return Err(format!(
                    "Model '{}' already exists in provider '{provider_id}'.",
                    model.id
                ));
            }

            settings.extra_models.push(model);
        } else {
            let cpd = data
                .custom_providers
                .get_mut(provider_id)
                .ok_or_else(|| format!("Custom provider '{provider_id}' not found."))?;

            if cpd.models.iter().any(|m| m.id == model.id) {
                return Err(format!(
                    "Model '{}' already exists in provider '{provider_id}'.",
                    model.id
                ));
            }

            cpd.models.push(model);
            self.registry.register_custom(cpd.to_definition())?;
        }

        self.save(&data)?;
        Ok(data)
    }

    /// Remove a model from a provider.
    pub fn remove_model(&self, provider_id: &str, model_id: &str) -> Result<ProvidersData, String> {
        let defn = self
            .registry
            .get(provider_id)
            .ok_or_else(|| format!("Provider '{provider_id}' not found."))?;

        let mut data = self.load()?;

        if defn.id == "ollama" {
            return Err("Cannot remove models from built-in provider 'ollama'. Ollama models are managed by the Ollama daemon itself.".to_string());
        }

        if BUILTIN_PROVIDER_IDS.contains(&provider_id) {
            if defn.models.iter().any(|m| m.id == model_id) {
                return Err(format!("Model '{model_id}' is a built-in model of '{provider_id}' and cannot be removed."));
            }

            let settings = data.providers.get_mut(provider_id).ok_or_else(|| {
                format!("Model '{model_id}' not found in provider '{provider_id}'.")
            })?;

            let original_len = settings.extra_models.len();
            settings.extra_models.retain(|m| m.id != model_id);

            if settings.extra_models.len() == original_len {
                return Err(format!(
                    "Model '{model_id}' not found in provider '{provider_id}'."
                ));
            }
        } else {
            let cpd = data
                .custom_providers
                .get_mut(provider_id)
                .ok_or_else(|| format!("Custom provider '{provider_id}' not found."))?;

            let original_len = cpd.models.len();
            cpd.models.retain(|m| m.id != model_id);

            if cpd.models.len() == original_len {
                return Err(format!(
                    "Model '{model_id}' not found in provider '{provider_id}'."
                ));
            }

            self.registry.register_custom(cpd.to_definition())?;
        }

        if data.active_llm.provider_id == provider_id && data.active_llm.model == model_id {
            data.active_llm = ModelSlotConfig::default();
        }

        self.save(&data)?;
        Ok(data)
    }
}

/// Mask API key for display.
pub fn mask_api_key(api_key: &str, visible_chars: usize) -> String {
    if api_key.is_empty() {
        return String::new();
    }
    if api_key.len() <= visible_chars {
        return "*".repeat(api_key.len());
    }
    let prefix = if api_key.len() > 3 { &api_key[..3] } else { "" };
    let suffix = &api_key[api_key.len() - visible_chars..];
    let hidden_len = api_key.len() - prefix.len() - visible_chars;
    format!("{}{}{}", prefix, "*".repeat(hidden_len.max(3)), suffix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_store(temp_dir: &TempDir) -> ProviderStore {
        let path = temp_dir.path().join("providers.json");
        let registry = Arc::new(ProviderRegistry::new());
        ProviderStore::new(path, registry)
    }

    #[test]
    fn test_load_empty() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);
        let data = store.load().unwrap();
        // Load now ensures built-in providers exist
        assert!(data.providers.len() >= 5); // At least openai, modelscope, dashscope, aliyun-codingplan, ollama
        assert!(data.custom_providers.is_empty());
        assert!(data.active_llm.is_empty());
    }

    #[test]
    fn test_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        let mut data = ProvidersData::default();
        data.active_llm = ModelSlotConfig::new("openai", "gpt-4");
        // Configure openai with an API key so active_llm won't be cleared on load
        data.providers.insert(
            "openai".to_string(),
            ProviderSettings {
                api_key: "sk-test-key".to_string(),
                ..Default::default()
            },
        );

        store.save(&data).unwrap();
        let loaded = store.load().unwrap();
        // Load now ensures built-in providers exist, so providers won't be empty
        assert!(!loaded.providers.is_empty());
        assert_eq!(loaded.active_llm.provider_id, "openai");
        assert_eq!(loaded.active_llm.model, "gpt-4");
    }

    #[test]
    fn test_ensure_builtin_providers() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        let data = store.load().unwrap();
        // Should have entries for all non-local built-in providers
        assert!(data.providers.contains_key("openai"));
        assert!(data.providers.contains_key("ollama"));
        // Local providers should not have entries
        assert!(!data.providers.contains_key("llamacpp"));
        assert!(!data.providers.contains_key("mlx"));
    }

    #[test]
    fn test_update_settings() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        let data = store
            .update_settings("openai", Some("sk-test".to_string()), None)
            .unwrap();
        assert_eq!(data.providers.get("openai").unwrap().api_key, "sk-test");

        let loaded = store.load().unwrap();
        assert_eq!(loaded.providers.get("openai").unwrap().api_key, "sk-test");
    }

    #[test]
    fn test_set_active_llm() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        let data = store.set_active_llm("openai", "gpt-4").unwrap();
        assert_eq!(data.active_llm.provider_id, "openai");
        assert_eq!(data.active_llm.model, "gpt-4");
    }

    #[test]
    fn test_create_custom_provider() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        let data = store
            .create_custom(
                "my-provider",
                "My Provider",
                "https://api.example.com/v1",
                "custom-",
                vec![],
            )
            .unwrap();

        assert!(data.custom_providers.contains_key("my-provider"));
        let cpd = &data.custom_providers["my-provider"];
        assert_eq!(cpd.name, "My Provider");
        assert_eq!(cpd.default_base_url, "https://api.example.com/v1");

        // Verify it's in the registry
        assert!(store.registry.get("my-provider").is_some());
    }

    #[test]
    fn test_delete_custom_provider() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        store
            .create_custom(
                "my-provider",
                "My Provider",
                "https://api.example.com/v1",
                "custom-",
                vec![],
            )
            .unwrap();

        let data = store.delete_custom("my-provider").unwrap();
        assert!(!data.custom_providers.contains_key("my-provider"));
        assert!(store.registry.get("my-provider").is_none());
    }

    #[test]
    fn test_add_remove_model() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        let model = ModelInfo::new("custom-model", "Custom Model");

        // Add model
        let data = store.add_model("openai", model.clone()).unwrap();
        assert!(data
            .providers
            .get("openai")
            .unwrap()
            .extra_models
            .iter()
            .any(|m| m.id == "custom-model"));

        // Remove model
        let data = store.remove_model("openai", "custom-model").unwrap();
        assert!(!data
            .providers
            .get("openai")
            .unwrap()
            .extra_models
            .iter()
            .any(|m| m.id == "custom-model"));
    }

    #[test]
    fn test_mask_api_key() {
        assert_eq!(mask_api_key("", 4), "");
        assert_eq!(mask_api_key("ab", 4), "**");
        assert_eq!(
            mask_api_key("sk-1234567890abcdef", 4),
            "sk-************cdef"
        );
        assert_eq!(mask_api_key("sk-test-key", 3), "sk-*****key");
    }

    #[test]
    fn test_is_configured() {
        let data = ProvidersData::default();
        let defn = ProviderDefinition {
            id: "llamacpp".to_string(),
            is_local: true,
            ..Default::default()
        };
        assert!(data.is_configured(&defn));

        let remote_defn = ProviderDefinition {
            id: "openai".to_string(),
            default_base_url: "https://api.openai.com/v1".to_string(),
            ..Default::default()
        };
        assert!(!data.is_configured(&remote_defn));

        // Built-in remote providers are configured once settings exist.
        let mut data = ProvidersData::default();
        data.providers.insert(
            "openai".to_string(),
            ProviderSettings {
                base_url: "https://api.openai.com/v1".to_string(),
                api_key: String::new(),
                extra_models: vec![],
                chat_model: String::new(),
            },
        );
        assert!(data.is_configured(&remote_defn));

        // Ollama requires base_url in settings.
        let ollama_defn = ProviderDefinition {
            id: "ollama".to_string(),
            ..Default::default()
        };
        assert!(!ProvidersData::default().is_configured(&ollama_defn));
    }
}
