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

    pub fn exclusions(&self) -> &[String] {
        &self.exclusions
    }

    /// Evaluates if a path should be indexed based on default and user exclusions.
    /// Excludes hidden files, git metadata, caches, build artifacts, and handles symlinks.
    pub fn should_index_path(&self, path: &Path) -> bool {
        self.should_index_relative(None, path)
    }

    /// Evaluates if a path relative to an approved root should be indexed.
    pub fn should_index_relative(&self, root: Option<&Path>, path: &Path) -> bool {
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

pub fn scan_directory(root: &Path, exclusions: &[String]) -> Result<Vec<FileMeta>> {
    if !root.exists() {
        return Err(SiftError::PathNotFound(root.to_string_lossy().to_string()));
    }
    if !root.is_dir() {
        return Err(SiftError::NotADirectory(root.to_string_lossy().to_string()));
    }

    let checker = PathRuleChecker::new(exclusions.to_vec());
    let mut files = Vec::new();

    let walker = walkdir::WalkDir::new(root).follow_links(false).into_iter();

    for entry in walker.filter_entry(|e| {
        let path = e.path();
        if path == root {
            return true;
        }
        checker.should_index_relative(Some(root), path)
    }) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if entry.file_type().is_file() {
            if let Ok(meta) = read_file_metadata(entry.path()) {
                files.push(meta);
            }
        }
    }

    Ok(files)
}

pub fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 {
            chunk[1] as usize
        } else {
            0
        };
        let b2 = if chunk.len() > 2 {
            chunk[2] as usize
        } else {
            0
        };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(triple >> 18) & 0x3F] as char);
        out.push(TABLE[(triple >> 12) & 0x3F] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(triple >> 6) & 0x3F] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[triple & 0x3F] as char);
        } else {
            out.push('=');
        }
    }
    out
}

pub fn get_image_mime(ext: &str) -> Option<&'static str> {
    match ext.to_lowercase().as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "svg" => Some("image/svg+xml"),
        "bmp" => Some("image/bmp"),
        "ico" => Some("image/x-icon"),
        _ => None,
    }
}

pub fn render_pdf_first_page(path: &Path) -> Option<String> {
    use std::process::Command;
    let output = Command::new("pdftoppm")
        .arg("-png")
        .arg("-f")
        .arg("1")
        .arg("-l")
        .arg("1")
        .arg("-scale-to")
        .arg("800")
        .arg(path)
        .output()
        .ok()?;

    if output.status.success() && !output.stdout.is_empty() {
        let b64 = base64_encode(&output.stdout);
        return Some(format!("data:image/png;base64,{}", b64));
    }
    None
}

pub fn extract_pdf_preview(path: &Path, max_bytes: usize) -> Option<String> {
    use std::process::Command;
    let output = Command::new("pdftotext")
        .arg("-l")
        .arg("5")
        .arg("-q")
        .arg(path)
        .arg("-")
        .output()
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !text.is_empty() {
            let mut truncated = text;
            if truncated.len() > max_bytes {
                truncated.truncate(max_bytes);
                truncated.push_str("\n\n[... preview truncated ...]");
            }
            return Some(truncated);
        }
    }
    None
}

pub fn read_file_preview(path: &Path, max_bytes: usize) -> Result<String> {
    if !path.exists() {
        return Err(SiftError::PathNotFound(path.to_string_lossy().to_string()));
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // 1. Image preview: encode as base64 data URL
    if let Some(mime) = get_image_mime(&ext) {
        use std::io::Read;
        let mut file = fs::File::open(path)?;
        // Limit preview image size to 10MB to avoid excessive RAM usage
        let max_img_bytes = 10 * 1024 * 1024;
        let mut buffer = Vec::new();
        let bytes_read = file
            .by_ref()
            .take(max_img_bytes as u64)
            .read_to_end(&mut buffer)?;
        if bytes_read == 0 {
            return Ok(String::new());
        }
        let b64 = base64_encode(&buffer);
        return Ok(format!("data:{};base64,{}", mime, b64));
    }

    // 2. PDF preview: first try visual render of page 1, fallback to text
    if ext == "pdf" {
        if let Some(visual_data_url) = render_pdf_first_page(path) {
            return Ok(visual_data_url);
        }
        if let Some(text) = extract_pdf_preview(path, max_bytes) {
            return Ok(text);
        }
        return Ok(
            "[PDF Document — Scanned or graphic document. Open with default viewer to read]"
                .to_string(),
        );
    }

    // 3. Regular text/code/data file preview
    use std::io::Read;
    let mut file = fs::File::open(path)?;
    let mut buffer = vec![0u8; max_bytes];
    let bytes_read = file.read(&mut buffer)?;
    buffer.truncate(bytes_read);

    // Check if content appears binary (e.g. contains null bytes)
    if buffer.contains(&0) {
        return Ok("[Binary content or unsupported encoding]".to_string());
    }

    match String::from_utf8(buffer) {
        Ok(text) => Ok(text),
        Err(e) => {
            let lossy = String::from_utf8_lossy(e.as_bytes()).to_string();
            Ok(lossy)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
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

    #[test]
    fn test_scan_directory_recursive() {
        let dir = Builder::new()
            .prefix("sift_test_")
            .tempdir()
            .expect("tempdir");
        let root = dir.path();

        let sub_dir = root.join("src");
        fs::create_dir_all(&sub_dir).expect("create subdir");
        let file1 = sub_dir.join("main.rs");
        File::create(&file1).expect("create main.rs");

        let git_dir = root.join(".git");
        fs::create_dir_all(&git_dir).expect("create .git");
        let git_file = git_dir.join("index");
        File::create(&git_file).expect("create git file");

        let files = scan_directory(root, &[".git".to_string()]).expect("scan directory");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].filename, "main.rs");
    }

    #[test]
    fn test_read_file_preview() {
        let dir = Builder::new()
            .prefix("sift_test_")
            .tempdir()
            .expect("tempdir");
        let text_file = dir.path().join("hello.txt");
        let mut f = File::create(&text_file).expect("create text file");
        f.write_all(b"Hello Sift Search!").expect("write text");

        let preview = read_file_preview(&text_file, 1024).expect("read preview");
        assert_eq!(preview, "Hello Sift Search!");

        let bin_file = dir.path().join("image.bin");
        let mut f_bin = File::create(&bin_file).expect("create bin file");
        f_bin
            .write_all(&[0x00, 0xFF, 0x00, 0xAA])
            .expect("write binary");

        let bin_preview = read_file_preview(&bin_file, 1024).expect("read bin preview");
        assert!(bin_preview.contains("Binary content"));
    }
}
