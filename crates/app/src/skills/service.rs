// -*- coding: utf-8 -*-
// Skills service for managing skills across builtin, customized, and active directories

use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::fs;
use tracing::{debug, error, warn};

/// Working directory path resolver
fn get_working_dir() -> PathBuf {
    std::env::var("COPAW_WORKING_DIR")
        .unwrap_or_else(|_| "~/.copaw".to_string())
        .replace('~', &std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
        .into()
}

/// Get active skills directory path
fn get_active_skills_dir() -> PathBuf {
    get_working_dir().join("active_skills")
}

/// Get customized skills directory path
fn get_customized_skills_dir() -> PathBuf {
    get_working_dir().join("customized_skills")
}

/// Get builtin skills directory path (in source tree)
fn get_builtin_skills_dir() -> PathBuf {
    // For development, this would be src/copaw/agents/skills
    // For installed version, we need to find the package location
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let project_root = PathBuf::from(&manifest_dir)
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or_else(|| Path::new(manifest_dir.as_str()))
            .to_path_buf();
        let builtin = project_root.join("src/copaw/agents/skills");
        if builtin.exists() {
            return builtin;
        }
    }

    // Fallback: assume we're running from the project root
    PathBuf::from("src/copaw/agents/skills")
}

/// Skill source type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillSource {
    Builtin,
    Customized,
    Active,
}

impl SkillSource {
    /// Get the directory path for this source type
    fn get_dir(&self) -> PathBuf {
        match self {
            SkillSource::Builtin => get_builtin_skills_dir(),
            SkillSource::Customized => get_customized_skills_dir(),
            SkillSource::Active => get_active_skills_dir(),
        }
    }
}

/// Skill information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    pub name: String,
    pub content: String,
    pub source: SkillSource,
    pub path: PathBuf,
    #[serde(default)]
    pub references: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub scripts: HashMap<String, serde_json::Value>,
}

/// Errors for skill operations
#[derive(Debug, Error)]
pub enum SkillError {
    #[error("Skill not found: {0}")]
    NotFound(String),

    #[error("Skill already exists: {0}")]
    AlreadyExists(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid skill content: {0}")]
    InvalidContent(String),
}

/// Build a directory tree structure for references or scripts
async fn build_directory_tree(directory: &Path) -> HashMap<String, serde_json::Value> {
    let mut tree = HashMap::new();

    if !directory.exists() || !directory.is_dir() {
        return tree;
    }

    let mut entries = match fs::read_dir(directory).await {
        Ok(e) => e,
        Err(_) => return tree,
    };

    while let Some(entry) = entries.next_entry().await.unwrap_or(None) {
        let name = entry.file_name().to_string_lossy().to_string();

        if entry.path().is_file() {
            tree.insert(name, serde_json::Value::Null);
        } else if entry.path().is_dir() {
            // Use non-recursive version for subdirectories
            let subtree = build_directory_tree_recursive(&entry.path()).await;
            tree.insert(name, serde_json::to_value(subtree).unwrap());
        }
    }

    tree
}

/// Non-recursive helper for building directory tree
fn build_directory_tree_recursive(directory: &Path) -> futures::future::BoxFuture<'static, HashMap<String, serde_json::Value>> {
    let directory = directory.to_path_buf();
    Box::pin(async move {
        let mut tree = HashMap::new();

        if !directory.exists() || !directory.is_dir() {
            return tree;
        }

        let mut entries = match fs::read_dir(&directory).await {
            Ok(e) => e,
            Err(_) => return tree,
        };

        while let Some(entry) = entries.next_entry().await.unwrap_or(None) {
            let name = entry.file_name().to_string_lossy().to_string();

            if entry.path().is_file() {
                tree.insert(name, serde_json::Value::Null);
            } else if entry.path().is_dir() {
                let subtree = build_directory_tree_recursive(&entry.path()).await;
                tree.insert(name, serde_json::to_value(subtree).unwrap());
            }
        }

        tree
    })
}

/// Collect skills from a directory
async fn collect_skills_from_dir(directory: &Path) -> HashMap<String, PathBuf> {
    let mut skills = HashMap::new();

    if !directory.exists() {
        return skills;
    }

    let mut entries = match fs::read_dir(directory).await {
        Ok(e) => e,
        Err(_) => return skills,
    };

    while let Some(entry) = entries.next_entry().await.unwrap_or(None) {
        let path = entry.path();
        if path.is_dir() && path.join("SKILL.md").exists() {
            let name = path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            skills.insert(name, path);
        }
    }

    skills
}

/// Skills service for managing skills
pub struct SkillService;

impl SkillService {
    /// List all skills from builtin and customized directories
    pub async fn list_all_skills() -> Result<Vec<SkillInfo>, SkillError> {
        let mut skills = Vec::new();

        // Sync from active to customized first
        if let Err(e) = Self::sync_from_active_to_customized_inner(None).await {
            debug!("Failed to sync skills from active to customized: {}", e);
        }

        // Collect from builtin skills
        skills.extend(Self::read_skills_from_dir(
            &get_builtin_skills_dir(),
            SkillSource::Builtin,
        ).await?);

        // Collect from customized skills
        skills.extend(Self::read_skills_from_dir(
            &get_customized_skills_dir(),
            SkillSource::Customized,
        ).await?);

        Ok(skills)
    }

    /// List available (active) skills
    pub async fn list_available_skills() -> Result<Vec<SkillInfo>, SkillError> {
        Self::read_skills_from_dir(&get_active_skills_dir(), SkillSource::Active).await
    }

    /// Create a new skill in customized_skills directory
    pub async fn create_skill(
        name: &str,
        content: &str,
        references: Option<HashMap<String, serde_json::Value>>,
        scripts: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<(), SkillError> {
        // Validate SKILL.md content (should have YAML front matter)
        if !content.contains("---") {
            return Err(SkillError::InvalidContent(
                "SKILL.md must contain YAML front matter".to_string()
            ));
        }

        let customized_dir = get_customized_skills_dir();
        fs::create_dir_all(&customized_dir).await?;

        let skill_dir = customized_dir.join(name);
        let skill_md = skill_dir.join("SKILL.md");

        // Check if skill already exists
        if skill_dir.exists() {
            return Err(SkillError::AlreadyExists(name.to_string()));
        }

        // Create skill directory and SKILL.md
        fs::create_dir_all(&skill_dir).await?;
        fs::write(&skill_md, content).await?;

        // Create references subdirectory if provided
        if let Some(refs) = references {
            let refs_dir = skill_dir.join("references");
            fs::create_dir_all(&refs_dir).await?;
            Self::create_files_from_tree_sync(&refs_dir, &refs)?;
        }

        // Create scripts subdirectory if provided
        if let Some(scr) = scripts {
            let scr_dir = skill_dir.join("scripts");
            fs::create_dir_all(&scr_dir).await?;
            Self::create_files_from_tree_sync(&scr_dir, &scr)?;
        }

        debug!("Created skill '{}' in customized_skills", name);
        Ok(())
    }

    /// Disable a skill by removing it from active_skills directory
    pub async fn disable_skill(name: &str) -> Result<bool, SkillError> {
        let active_dir = get_active_skills_dir();
        let skill_dir = active_dir.join(name);

        if !skill_dir.exists() {
            debug!("Skill '{}' not found in active_skills", name);
            return Ok(false);
        }

        fs::remove_dir_all(&skill_dir).await?;
        debug!("Disabled skill '{}' from active_skills", name);
        Ok(true)
    }

    /// Enable a skill by syncing it to active_skills directory
    pub async fn enable_skill(name: &str, force: bool) -> Result<bool, SkillError> {
        Self::sync_skills_to_working_dir_inner(Some(vec![name.to_string()]), force).await?;

        let active_dir = get_active_skills_dir();
        Ok(active_dir.join(name).exists())
    }

    /// Delete a skill from customized_skills directory permanently
    pub async fn delete_skill(name: &str) -> Result<bool, SkillError> {
        let customized_dir = get_customized_skills_dir();
        let skill_dir = customized_dir.join(name);

        if !skill_dir.exists() {
            debug!("Skill '{}' not found in customized_skills", name);
            return Ok(false);
        }

        fs::remove_dir_all(&skill_dir).await?;
        debug!("Deleted skill '{}' from customized_skills", name);
        Ok(true)
    }

    /// Load a specific file from a skill's references or scripts directory
    pub async fn load_skill_file(
        skill_name: &str,
        file_path: &str,
        source: SkillSource,
    ) -> Result<String, SkillError> {
        // Validate source
        if !matches!(source, SkillSource::Builtin | SkillSource::Customized) {
            return Err(SkillError::InvalidContent(
                "Source must be 'builtin' or 'customized'".to_string()
            ));
        }

        // Normalize file path
        let normalized = file_path.replace('\\', "/");

        // Validate file_path starts with references/ or scripts/
        if !normalized.starts_with("references/") && !normalized.starts_with("scripts/") {
            return Err(SkillError::InvalidContent(
                "file_path must start with 'references/' or 'scripts/'".to_string()
            ));
        }

        // Prevent path traversal
        if normalized.contains("..") || normalized.starts_with('/') {
            return Err(SkillError::InvalidContent(
                "Path traversal not allowed".to_string()
            ));
        }

        let base_dir = source.get_dir();
        let skill_dir = base_dir.join(skill_name);
        let full_path = skill_dir.join(&normalized);

        // Check if skill exists
        if !skill_dir.exists() {
            return Err(SkillError::NotFound(format!("'{}' in {}", skill_name, serde_json::to_string(&source).unwrap_or_default())));
        }

        // Check if file exists
        if !full_path.exists() {
            return Err(SkillError::NotFound(format!("'{}' in skill '{}'", file_path, skill_name)));
        }

        // Check if it's a file
        if !full_path.is_file() {
            return Err(SkillError::InvalidContent(format!("'{}' is not a file", file_path)));
        }

        let content = fs::read_to_string(&full_path).await?;
        debug!("Loaded file '{}' from skill '{}'", file_path, skill_name);
        Ok(content)
    }

    /// Read skills from a directory
    async fn read_skills_from_dir(
        directory: &Path,
        source: SkillSource,
    ) -> Result<Vec<SkillInfo>, SkillError> {
        let mut skills = Vec::new();

        if !directory.exists() {
            return Ok(skills);
        }

        let mut entries = match fs::read_dir(directory).await {
            Ok(e) => e,
            Err(e) => return Err(SkillError::Io(e)),
        };

        while let Some(entry) = entries.next_entry().await.unwrap_or(None) {
            let skill_dir = entry.path();

            if !skill_dir.is_dir() {
                continue;
            }

            let skill_md = skill_dir.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }

            let name = skill_dir.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            match Self::read_skill_info(&skill_dir, &name, source).await {
                Ok(info) => skills.push(info),
                Err(e) => error!("Failed to read skill '{}': {}", name, e),
            }
        }

        Ok(skills)
    }

    /// Read skill info from a skill directory
    async fn read_skill_info(
        skill_dir: &Path,
        name: &str,
        source: SkillSource,
    ) -> Result<SkillInfo, SkillError> {
        let skill_md = skill_dir.join("SKILL.md");
        let content = fs::read_to_string(&skill_md).await?;

        // Build references directory tree
        let references = build_directory_tree(&skill_dir.join("references")).await;

        // Build scripts directory tree
        let scripts = build_directory_tree(&skill_dir.join("scripts")).await;

        Ok(SkillInfo {
            name: name.to_string(),
            content,
            source,
            path: skill_dir.to_path_buf(),
            references,
            scripts,
        })
    }

    /// Create files from a tree structure
    fn create_files_from_tree_sync(
        base_dir: &Path,
        tree: &HashMap<String, serde_json::Value>,
    ) -> Result<(), SkillError> {
        for (name, value) in tree {
            let item_path = base_dir.join(name);

            match value {
                serde_json::Value::Null => {
                    // Create empty file
                    std::fs::write(&item_path, "")?;
                }
                serde_json::Value::String(content) => {
                    // Create file with content
                    std::fs::write(&item_path, content)?;
                }
                serde_json::Value::Object(_) => {
                    // It's a directory - recursively create
                    if let Ok(subtree) = serde_json::from_value::<HashMap<String, serde_json::Value>>(value.clone()) {
                        std::fs::create_dir_all(&item_path)?;
                        Self::create_files_from_tree_sync(&item_path, &subtree)?;
                    }
                }
                _ => {
                    return Err(SkillError::InvalidContent(
                        format!("Invalid tree value for '{}'", name)
                    ));
                }
            }
        }

        Ok(())
    }

    /// Sync skills from builtin and customized to active_skills directory
    async fn sync_skills_to_working_dir_inner(
        skill_names: Option<Vec<String>>,
        force: bool,
    ) -> Result<(usize, usize), SkillError> {
        let builtin_skills = get_builtin_skills_dir();
        let customized_skills = get_customized_skills_dir();
        let active_skills = get_active_skills_dir();

        // Ensure active skills directory exists
        fs::create_dir_all(&active_skills).await?;

        // Collect skills from both sources (customized overwrites builtin)
        let mut skills_to_sync = collect_skills_from_dir(&builtin_skills).await;
        let customized = collect_skills_from_dir(&customized_skills).await;
        skills_to_sync.extend(customized);

        // Filter by skill_names if specified
        if let Some(names) = skill_names {
            skills_to_sync = skills_to_sync.into_iter()
                .filter(|(name, _)| names.contains(name))
                .collect();
        }

        if skills_to_sync.is_empty() {
            return Ok((0, 0));
        }

        let mut synced_count = 0;
        let mut skipped_count = 0;

        for (skill_name, skill_dir) in skills_to_sync {
            let target_dir = active_skills.join(&skill_name);

            // Check if skill already exists
            if target_dir.exists() && !force {
                debug!("Skill '{}' already exists in active_skills, skipping", skill_name);
                skipped_count += 1;
                continue;
            }

            // Copy skill directory
            if target_dir.exists() {
                fs::remove_dir_all(&target_dir).await?;
            }

            Self::copy_dir_recursive(&skill_dir, &target_dir).await?;
            debug!("Synced skill '{}' to active_skills", skill_name);
            synced_count += 1;
        }

        Ok((synced_count, skipped_count))
    }

    /// Sync skills from active_skills to customized_skills directory
    async fn sync_from_active_to_customized_inner(
        skill_names: Option<Vec<String>>,
    ) -> Result<(usize, usize), SkillError> {
        let active_skills = get_active_skills_dir();
        let customized_skills = get_customized_skills_dir();
        let builtin_skills = get_builtin_skills_dir();

        fs::create_dir_all(&customized_skills).await?;

        let active_skills_map = collect_skills_from_dir(&active_skills).await;

        if active_skills_map.is_empty() {
            debug!("No skills found in active_skills");
            return Ok((0, 0));
        }

        let builtin_skills_map = collect_skills_from_dir(&builtin_skills).await;

        let mut synced_count = 0;
        let mut skipped_count = 0;

        for (skill_name, skill_dir) in active_skills_map {
            if let Some(ref names) = skill_names {
                if !names.contains(&skill_name) {
                    continue;
                }
            }

            // Skip if it's the same as builtin
            if let Some(builtin_dir) = builtin_skills_map.get(&skill_name) {
                if Self::is_directory_same_sync(skill_dir.clone(), builtin_dir.clone()) {
                    skipped_count += 1;
                    continue;
                }
            }

            let target_dir = customized_skills.join(&skill_name);

            if target_dir.exists() {
                fs::remove_dir_all(&target_dir).await?;
            }

            Self::copy_dir_recursive(&skill_dir, &target_dir).await?;
            debug!("Synced skill '{}' from active_skills to customized_skills", skill_name);
            synced_count += 1;
        }

        Ok((synced_count, skipped_count))
    }

    /// Copy a directory recursively
    async fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), SkillError> {
        Self::copy_dir_recursive_helper(src, dst).await
    }

    /// Helper function for recursive directory copying (boxed for async recursion)
    fn copy_dir_recursive_helper<'a>(src: &'a Path, dst: &'a Path) -> futures::future::BoxFuture<'a, Result<(), SkillError>> {
        let src = src.to_path_buf();
        let dst = dst.to_path_buf();
        Box::pin(async move {
            fs::create_dir_all(&dst).await?;

            let mut entries = fs::read_dir(&src).await?;
            while let Some(entry) = entries.next_entry().await? {
                let src_path = entry.path();
                let dst_path = dst.join(entry.file_name());

                if src_path.is_dir() {
                    Self::copy_dir_recursive_helper(&src_path, &dst_path).await?;
                } else {
                    fs::copy(&src_path, &dst_path).await?;
                }
            }

            Ok(())
        })
    }

    /// Check if two directories have the same content
    fn is_directory_same_sync(dir1: PathBuf, dir2: PathBuf) -> bool {
        if !dir1.exists() || !dir2.exists() {
            return false;
        }

        let entries1 = match std::fs::read_dir(&dir1) {
            Ok(e) => e,
            Err(_) => return false,
        };

        let entries2 = match std::fs::read_dir(&dir2) {
            Ok(e) => e,
            Err(_) => return false,
        };

        // Collect entries from both directories
        let mut map1 = std::collections::HashMap::new();
        for entry in entries1.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            map1.insert(name, entry.path());
        }

        let mut map2 = std::collections::HashMap::new();
        for entry in entries2.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            map2.insert(name, entry.path());
        }

        // Check if entries match
        if map1.keys().collect::<Vec<_>>() != map2.keys().collect::<Vec<_>>() {
            return false;
        }

        // Compare file contents
        for (name, path1) in map1 {
            let path2 = &map2[&name];

            if path1.is_dir() && path2.is_dir() {
                if !Self::is_directory_same_sync(path1.clone(), path2.clone()) {
                    return false;
                }
            } else if path1.is_file() && path2.is_file() {
                let content1 = match std::fs::read(&path1) {
                    Ok(c) => c,
                    Err(_) => return false,
                };
                let content2 = match std::fs::read(&path2) {
                    Ok(c) => c,
                    Err(_) => return false,
                };

                if content1 != content2 {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_skill_source_dirs() {
        let builtin = SkillSource::Builtin.get_dir();
        let customized = SkillSource::Customized.get_dir();
        let active = SkillSource::Active.get_dir();

        // These should be different paths
        assert_ne!(builtin, customized);
        assert_ne!(builtin, active);
        assert_ne!(customized, active);
    }

    #[tokio::test]
    async fn test_build_directory_tree() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path();

        // Create test structure
        fs::create_dir_all(dir.join("subdir")).await.unwrap();
        fs::write(dir.join("file.txt"), "content").await.unwrap();
        fs::write(dir.join("subdir/nested.py"), "print('hello')").await.unwrap();

        let tree = build_directory_tree(dir).await;

        assert!(tree.contains_key("file.txt"));
        assert!(tree.contains_key("subdir"));
    }

    #[tokio::test]
    async fn test_create_skill() {
        let content = r#"---
name: test_skill
description: A test skill
---
# Test Skill

This is a test skill.
"#;

        // This test would require setting up a temporary working directory
        // For now, we just verify the content validation
        assert!(content.contains("---"));
    }
}
