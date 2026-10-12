use crate::error::{Result, SiftError};
use chrono::{DateTime, Utc};
use std::fs;
use std::path::Path;

pub struct PathRuleChecker {
    exclusions: Vec<String>,
}

impl PathRuleChecker {
    pub fn new(exclusions: Vec<String>) -> Self {
        Self { exclusions }
    }

    /// Evaluates if a path should be indexed based on default and user exclusions.
    /// Excludes hidden files, git metadata, caches, build artifacts, and handles symlinks.
    pub fn should_index_path(&self, path: &Path) -> bool {
        self.should_index_relative(None, path)
    }

    /// Evaluates if a path relative to an approved root should be indexed.
    pub fn should_index_relative(&self, root: Option<&Path>, path: &Path) -> bool {
        // Must exist
        if !path.exists() {
            return false;
        }

        // Avoid following symlinks to arbitrary directories (prevents traversal loops)
        if let Ok(metadata) = fs::symlink_metadata(path) {
            if metadata.file_type().is_symlink() {
                return false;
            }
        }

        // If root is provided, examine path relative to root; otherwise examine all components
        let examine_path = if let Some(r) = root {
            path.strip_prefix(r).unwrap_or(path)
        } else {
            path
        };

        for component in examine_path.components() {
            let comp_str = component.as_os_str().to_string_lossy();

            // Exclude hidden directories/files (starting with .) except current/root
            if comp_str.starts_with('.') && comp_str != "." && comp_str != ".." {
                return false;
            }

            // Check against configured exclusion patterns
            for pattern in &self.exclusions {
                if comp_str == pattern.as_str() || comp_str.ends_with(pattern.as_str()) {
                    return false;
                }
            }
        }

        true
    }
}

pub struct FileMeta {
    pub path: String,
    pub filename: String,
    pub extension: String,
    pub size_bytes: i64,
    pub modified_at: DateTime<Utc>,
    pub fingerprint: String,
}

pub fn read_file_metadata(path: &Path) -> Result<FileMeta> {
    if !path.exists() {
        return Err(SiftError::PathNotFound(path.to_string_lossy().to_string()));
    }

    let meta = fs::metadata(path)?;
    let size_bytes = meta.len() as i64;

    let modified_at = meta
        .modified()
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(|_| Utc::now());

    let filename = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let extension = path
        .extension()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    // Fingerprint: size + mtime_secs + nanosecs
    let mtime_secs = modified_at.timestamp();
    let mtime_nanos = modified_at.timestamp_subsec_nanos();
    let fingerprint = format!("{}:{}:{}", size_bytes, mtime_secs, mtime_nanos);

    Ok(FileMeta {
        path: path.to_string_lossy().to_string(),
        filename,
        extension,
        size_bytes,
        modified_at,
        fingerprint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::Builder;

    #[test]
    fn test_exclusion_rules() {
        let checker = PathRuleChecker::new(vec![
            "node_modules".to_string(),
            "target".to_string(),
            ".git".to_string(),
            "dist".to_string(),
        ]);

        let dir = Builder::new()
            .prefix("sift_test_")
            .tempdir()
            .expect("tempdir");
        let valid_file = dir.path().join("readme.md");
        File::create(&valid_file).expect("create file");

        let git_dir = dir.path().join(".git");
        fs::create_dir_all(&git_dir).expect("create .git");
        let git_file = git_dir.join("config");
        File::create(&git_file).expect("create git config");

        let node_dir = dir.path().join("node_modules").join("package");
        fs::create_dir_all(&node_dir).expect("create node_modules");
        let node_file = node_dir.join("index.js");
        File::create(&node_file).expect("create node file");

        let hidden_file = dir.path().join(".env");
        File::create(&hidden_file).expect("create hidden file");

        assert!(checker.should_index_relative(Some(dir.path()), &valid_file));
        assert!(!checker.should_index_relative(Some(dir.path()), &git_file));
        assert!(!checker.should_index_relative(Some(dir.path()), &node_file));
        assert!(!checker.should_index_relative(Some(dir.path()), &hidden_file));
    }

    #[test]
    fn test_file_metadata_reader() {
        let dir = Builder::new()
            .prefix("sift_test_")
            .tempdir()
            .expect("tempdir");
        let file_path = dir.path().join("document.pdf");
        File::create(&file_path).expect("create file");

        let meta = read_file_metadata(&file_path).expect("read metadata");
        assert_eq!(meta.filename, "document.pdf");
        assert_eq!(meta.extension, "pdf");
        assert_eq!(meta.size_bytes, 0);
        assert!(!meta.fingerprint.is_empty());
    }
}
