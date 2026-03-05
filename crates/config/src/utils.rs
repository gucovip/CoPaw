use std::path::PathBuf;

/// Expand home directory tilde (~) in path
pub fn expand_home(path: &str) -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        if path.starts_with('~') {
            // Skip ~ and any following /
            let rest = path
                .strip_prefix('~')
                .unwrap_or(path)
                .trim_start_matches('/');
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

/// Get working directory (~/.copaw/)
pub fn get_working_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        return home.join(".copaw");
    }
    PathBuf::from(".copaw")
}

/// Get chats directory (~/.copaw/chats/)
pub fn get_chats_path() -> PathBuf {
    get_working_dir().join("chats")
}

/// Get jobs directory (~/.copaw/jobs/)
pub fn get_jobs_path() -> PathBuf {
    get_working_dir().join("jobs")
}

/// Get skills directory (~/.copaw/skills/)
pub fn get_skills_path() -> PathBuf {
    get_working_dir().join("skills")
}

/// Get media directory (~/.copaw/media/)
pub fn get_media_path() -> PathBuf {
    get_working_dir().join("media")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_home() {
        let home = dirs::home_dir().expect("No home dir");
        let expanded = expand_home("~/test");
        assert_eq!(expanded, home.join("test"));
    }

    #[test]
    fn test_expand_no_tilde() {
        let expanded = expand_home("/absolute/path");
        assert_eq!(expanded, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_get_working_dir() {
        let wd = get_working_dir();
        assert!(wd.ends_with(".copaw"));
    }

    #[test]
    fn test_get_chats_path() {
        let path = get_chats_path();
        assert!(path.ends_with("chats"));
    }
}
