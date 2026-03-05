//! Agent file manager for working with markdown files.
//!
//! Manages markdown files in the working directory and memory subdirectory.
//! Similar to the Python AgentMdManager.

use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors that can occur in file manager operations.
#[derive(Debug, Error)]
pub enum AgentFileManagerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

/// Metadata for a markdown file.
#[derive(Debug, Clone)]
pub struct MdFileMetadata {
    /// File name (with .md extension)
    pub filename: String,
    /// Full file path
    pub path: PathBuf,
    /// Size in bytes
    pub size: i64,
    /// Created time (ISO8601)
    pub created_time: String,
    /// Modified time (ISO8601)
    pub modified_time: String,
}

/// Agent file manager for working with markdown files.
///
/// Manages markdown files in two directories:
/// - Working directory: ~/.copaw/ (top level)
/// - Memory directory: ~/.copaw/memory/
#[derive(Clone)]
pub struct AgentFileManager {
    /// Working directory path
    working_dir: PathBuf,
    /// Memory directory path (subdirectory of working_dir)
    memory_dir: PathBuf,
}

impl AgentFileManager {
    /// Create a new file manager with the default working directory (~/.copaw/).
    pub fn new() -> Result<Self, AgentFileManagerError> {
        let working_dir = dirs::home_dir()
            .map(|d| d.join(".copaw"))
            .unwrap_or_else(|| PathBuf::from(".copaw"));

        Self::with_working_dir(working_dir)
    }

    /// Create a new file manager with a custom working directory.
    pub fn with_working_dir<P: AsRef<Path>>(working_dir: P) -> Result<Self, AgentFileManagerError> {
        let working_dir = working_dir.as_ref().to_path_buf();
        let memory_dir = working_dir.join("memory");

        // Ensure directories exist
        std::fs::create_dir_all(&working_dir)?;
        std::fs::create_dir_all(&memory_dir)?;

        Ok(Self {
            working_dir,
            memory_dir,
        })
    }

    /// Get the working directory path.
    pub fn working_dir(&self) -> &Path {
        &self.working_dir
    }

    /// Get the memory directory path.
    pub fn memory_dir(&self) -> &Path {
        &self.memory_dir
    }

    /// List all markdown files in the working directory.
    pub fn list_working_mds(&self) -> Result<Vec<MdFileMetadata>, AgentFileManagerError> {
        self.list_mds_in_dir(&self.working_dir)
    }

    /// Read a markdown file from the working directory.
    ///
    /// Automatically appends .md extension if not present.
    pub fn read_working_md(&self, md_name: &str) -> Result<String, AgentFileManagerError> {
        let file_path = self.resolve_working_path(md_name)?;
        if !file_path.exists() {
            return Err(AgentFileManagerError::FileNotFound(
                file_path.display().to_string(),
            ));
        }
        Ok(std::fs::read_to_string(&file_path)?)
    }

    /// Write markdown content to a file in the working directory.
    ///
    /// Automatically appends .md extension if not present.
    pub fn write_working_md(
        &self,
        md_name: &str,
        content: &str,
    ) -> Result<(), AgentFileManagerError> {
        let file_path = self.resolve_working_path(md_name)?;
        std::fs::write(&file_path, content)?;
        Ok(())
    }

    /// Delete a markdown file from the working directory.
    ///
    /// Automatically appends .md extension if not present.
    pub fn delete_working_md(&self, md_name: &str) -> Result<(), AgentFileManagerError> {
        let file_path = self.resolve_working_path(md_name)?;
        if !file_path.exists() {
            return Err(AgentFileManagerError::FileNotFound(
                file_path.display().to_string(),
            ));
        }
        std::fs::remove_file(&file_path)?;
        Ok(())
    }

    /// List all markdown files in the memory directory.
    pub fn list_memory_mds(&self) -> Result<Vec<MdFileMetadata>, AgentFileManagerError> {
        self.list_mds_in_dir(&self.memory_dir)
    }

    /// Read a markdown file from the memory directory.
    ///
    /// Automatically appends .md extension if not present.
    pub fn read_memory_md(&self, md_name: &str) -> Result<String, AgentFileManagerError> {
        let file_path = self.resolve_memory_path(md_name)?;
        if !file_path.exists() {
            return Err(AgentFileManagerError::FileNotFound(
                file_path.display().to_string(),
            ));
        }
        Ok(std::fs::read_to_string(&file_path)?)
    }

    /// Write markdown content to a file in the memory directory.
    ///
    /// Automatically appends .md extension if not present.
    pub fn write_memory_md(
        &self,
        md_name: &str,
        content: &str,
    ) -> Result<(), AgentFileManagerError> {
        let file_path = self.resolve_memory_path(md_name)?;
        std::fs::write(&file_path, content)?;
        Ok(())
    }

    /// Delete a markdown file from the memory directory.
    ///
    /// Automatically appends .md extension if not present.
    pub fn delete_memory_md(&self, md_name: &str) -> Result<(), AgentFileManagerError> {
        let file_path = self.resolve_memory_path(md_name)?;
        if !file_path.exists() {
            return Err(AgentFileManagerError::FileNotFound(
                file_path.display().to_string(),
            ));
        }
        std::fs::remove_file(&file_path)?;
        Ok(())
    }

    /// Resolve the full path for a working directory file.
    fn resolve_working_path(&self, md_name: &str) -> Result<PathBuf, AgentFileManagerError> {
        let filename = if md_name.ends_with(".md") {
            md_name.to_string()
        } else {
            format!("{}.md", md_name)
        };
        Ok(self.working_dir.join(filename))
    }

    /// Resolve the full path for a memory directory file.
    fn resolve_memory_path(&self, md_name: &str) -> Result<PathBuf, AgentFileManagerError> {
        let filename = if md_name.ends_with(".md") {
            md_name.to_string()
        } else {
            format!("{}.md", md_name)
        };
        Ok(self.memory_dir.join(filename))
    }

    /// List markdown files in a directory with metadata.
    fn list_mds_in_dir(&self, dir: &Path) -> Result<Vec<MdFileMetadata>, AgentFileManagerError> {
        let mut files = Vec::new();

        let entries = std::fs::read_dir(dir).map_err(|e| AgentFileManagerError::Io(e))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Only include .md files that are actual files (not directories)
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                let metadata = entry.metadata()?;
                let stat = FileStat::from_metadata(&metadata);

                files.push(MdFileMetadata {
                    filename: path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    path: path.clone(),
                    size: stat.size,
                    created_time: stat.created_time,
                    modified_time: stat.modified_time,
                });
            }
        }

        // Sort by filename
        files.sort_by(|a, b| a.filename.cmp(&b.filename));

        Ok(files)
    }
}

impl Default for AgentFileManager {
    fn default() -> Self {
        Self::new().expect("Failed to create default agent file manager")
    }
}

/// File statistics wrapper.
struct FileStat {
    size: i64,
    created_time: String,
    modified_time: String,
}

impl FileStat {
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;

        Self {
            size: metadata.len() as i64,
            created_time: {
                // On Unix, use ctime (change time) as creation time
                let secs = metadata.ctime();
                format_datetime(secs)
            },
            modified_time: {
                let secs = metadata.mtime();
                format_datetime(secs)
            },
        }
    }
}

/// Format Unix timestamp to ISO8601 string.
fn format_datetime(secs: i64) -> String {
    use chrono::{TimeZone, Utc};
    Utc.timestamp_opt(secs, 0)
        .single()
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_file_manager_create() {
        let temp_dir = TempDir::new().unwrap();
        let fm = AgentFileManager::with_working_dir(temp_dir.path()).unwrap();
        assert_eq!(fm.working_dir(), temp_dir.path());
        assert_eq!(fm.memory_dir(), temp_dir.path().join("memory"));
    }

    #[test]
    fn test_write_and_read_working_md() {
        let temp_dir = TempDir::new().unwrap();
        let fm = AgentFileManager::with_working_dir(temp_dir.path()).unwrap();

        fm.write_working_md("test", "# Test Content").unwrap();
        let content = fm.read_working_md("test").unwrap();
        assert_eq!(content, "# Test Content");
    }

    #[test]
    fn test_write_and_read_working_md_with_extension() {
        let temp_dir = TempDir::new().unwrap();
        let fm = AgentFileManager::with_working_dir(temp_dir.path()).unwrap();

        fm.write_working_md("test.md", "# Test Content").unwrap();
        let content = fm.read_working_md("test.md").unwrap();
        assert_eq!(content, "# Test Content");
    }

    #[test]
    fn test_read_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let fm = AgentFileManager::with_working_dir(temp_dir.path()).unwrap();

        let result = fm.read_working_md("nonexistent");
        assert!(matches!(
            result,
            Err(AgentFileManagerError::FileNotFound(_))
        ));
    }

    #[test]
    fn test_list_working_mds() {
        let temp_dir = TempDir::new().unwrap();
        let fm = AgentFileManager::with_working_dir(temp_dir.path()).unwrap();

        fm.write_working_md("file1.md", "content1").unwrap();
        fm.write_working_md("file2.md", "content2").unwrap();

        let files = fm.list_working_mds().unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].filename, "file1.md");
        assert_eq!(files[1].filename, "file2.md");
    }

    #[test]
    fn test_write_and_read_memory_md() {
        let temp_dir = TempDir::new().unwrap();
        let fm = AgentFileManager::with_working_dir(temp_dir.path()).unwrap();

        fm.write_memory_md("memory_test", "# Memory Content")
            .unwrap();
        let content = fm.read_memory_md("memory_test").unwrap();
        assert_eq!(content, "# Memory Content");
    }

    #[test]
    fn test_list_memory_mds() {
        let temp_dir = TempDir::new().unwrap();
        let fm = AgentFileManager::with_working_dir(temp_dir.path()).unwrap();

        fm.write_memory_md("mem1.md", "mem1").unwrap();
        fm.write_memory_md("mem2.md", "mem2").unwrap();

        let files = fm.list_memory_mds().unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_delete_working_md() {
        let temp_dir = TempDir::new().unwrap();
        let fm = AgentFileManager::with_working_dir(temp_dir.path()).unwrap();

        fm.write_working_md("to_delete", "content").unwrap();
        fm.delete_working_md("to_delete").unwrap();

        let result = fm.read_working_md("to_delete");
        assert!(matches!(
            result,
            Err(AgentFileManagerError::FileNotFound(_))
        ));
    }

    #[test]
    fn test_delete_memory_md() {
        let temp_dir = TempDir::new().unwrap();
        let fm = AgentFileManager::with_working_dir(temp_dir.path()).unwrap();

        fm.write_memory_md("to_delete", "content").unwrap();
        fm.delete_memory_md("to_delete").unwrap();

        let result = fm.read_memory_md("to_delete");
        assert!(matches!(
            result,
            Err(AgentFileManagerError::FileNotFound(_))
        ));
    }

    #[test]
    fn test_resolve_working_path() {
        let temp_dir = TempDir::new().unwrap();
        let fm = AgentFileManager::with_working_dir(temp_dir.path()).unwrap();

        let path = fm.resolve_working_path("test").unwrap();
        assert!(path.ends_with("test.md"));

        let path = fm.resolve_working_path("test.md").unwrap();
        assert!(path.ends_with("test.md"));
    }

    #[test]
    fn test_resolve_memory_path() {
        let temp_dir = TempDir::new().unwrap();
        let fm = AgentFileManager::with_working_dir(temp_dir.path()).unwrap();

        let path = fm.resolve_memory_path("test").unwrap();
        assert!(path.ends_with("memory/test.md"));

        let path = fm.resolve_memory_path("test.md").unwrap();
        assert!(path.ends_with("memory/test.md"));
    }

    #[test]
    fn test_file_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let fm = AgentFileManager::with_working_dir(temp_dir.path()).unwrap();

        fm.write_working_md("metadata_test", "some content")
            .unwrap();
        let files = fm.list_working_mds().unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].filename, "metadata_test.md");
        assert!(files[0].size > 0);
        assert!(!files[0].created_time.is_empty());
        assert!(!files[0].modified_time.is_empty());
    }
}
