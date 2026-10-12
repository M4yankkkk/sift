use crate::error::{Result, SiftError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::Command;

pub const MAX_INDEXABLE_FILE_SIZE_BYTES: u64 = 10 * 1024 * 1024; // 10 MB

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentChunk {
    pub chunk_index: u32,
    pub page_number: Option<i32>,
    pub line_number: Option<i32>,
    pub content: String,
}

/// Checks if an extension is commonly a readable text, code, or data file.
pub fn is_text_or_code_extension(ext: &str) -> bool {
    matches!(
        ext.to_lowercase().as_str(),
        "txt"
            | "log"
            | "csv"
            | "tsv"
            | "md"
            | "markdown"
            | "rst"
            | "rs"
            | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "py"
            | "c"
            | "cpp"
            | "cc"
            | "h"
            | "hpp"
            | "go"
            | "java"
            | "kt"
            | "swift"
            | "scala"
            | "rb"
            | "php"
            | "html"
            | "htm"
            | "css"
            | "scss"
            | "sass"
            | "less"
            | "json"
            | "json5"
            | "toml"
            | "yaml"
            | "yml"
            | "sh"
            | "bash"
            | "zsh"
            | "fish"
            | "sql"
            | "xml"
            | "svg"
            | "graphql"
            | "gql"
            | "dockerfile"
            | "ini"
            | "conf"
            | "cfg"
            | "env"
    )
}

/// Extracts text and line-bounded chunks from plain text, Markdown, or source code files.
pub fn extract_text_chunks(path: &Path, max_bytes: u64) -> Result<Vec<DocumentChunk>> {
    let metadata = fs::metadata(path).map_err(SiftError::Io)?;
    if metadata.len() > max_bytes {
        tracing::debug!(
            "Skipping full-text extraction: file {:?} exceeds size limit ({} bytes)",
            path,
            metadata.len()
        );
        return Ok(Vec::new());
    }

    let mut file = fs::File::open(path).map_err(SiftError::Io)?;

    // Check first 1024 bytes for null character to avoid indexing binary files
    let mut probe = [0u8; 1024];
    let probe_read = file.read(&mut probe).map_err(SiftError::Io)?;
    if probe[..probe_read].contains(&0) {
        tracing::debug!("Skipping binary file with null bytes: {:?}", path);
        return Ok(Vec::new());
    }

    // Read full content up to size limit
    let mut buffer = Vec::new();
    buffer.extend_from_slice(&probe[..probe_read]);
    file.read_to_end(&mut buffer).map_err(SiftError::Io)?;

    let text = match String::from_utf8(buffer) {
        Ok(s) => s,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).to_string(),
    };

    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return Ok(Vec::new());
    }

    // If small (< 60 lines), return a single chunk
    if lines.len() <= 60 {
        return Ok(vec![DocumentChunk {
            chunk_index: 0,
            page_number: None,
            line_number: Some(1),
            content: text,
        }]);
    }

    // Otherwise, chunk into overlapping blocks of 50 lines with 10-line overlap
    let chunk_size = 50;
    let overlap = 10;
    let mut chunks = Vec::new();
    let mut start_idx = 0;
    let mut chunk_idx = 0;

    while start_idx < lines.len() {
        let end_idx = (start_idx + chunk_size).min(lines.len());
        let chunk_text = lines[start_idx..end_idx].join("\n");
        let start_line_num = (start_idx + 1) as i32;

        chunks.push(DocumentChunk {
            chunk_index: chunk_idx,
            page_number: None,
            line_number: Some(start_line_num),
            content: chunk_text,
        });

        chunk_idx += 1;
        if end_idx >= lines.len() {
            break;
        }
        start_idx += chunk_size - overlap;
    }

    Ok(chunks)
}

/// Extracts page-by-page text from a PDF document using poppler's `pdftotext`.
/// Pages in pdftotext output are delimited by the form feed character `\x0c`.
pub fn extract_pdf_chunks(path: &Path, max_bytes: u64) -> Result<Vec<DocumentChunk>> {
    let metadata = fs::metadata(path).map_err(SiftError::Io)?;
    if metadata.len() > max_bytes {
        tracing::debug!(
            "Skipping PDF extraction: file {:?} exceeds size limit ({} bytes)",
            path,
            metadata.len()
        );
        return Ok(Vec::new());
    }

    let output = match Command::new("pdftotext")
        .arg("-q")
        .arg(path)
        .arg("-")
        .output()
    {
        Ok(out) => out,
        Err(e) => {
            tracing::warn!("pdftotext command failed to execute: {}", e);
            return Ok(Vec::new());
        }
    };

    if !output.status.success() {
        tracing::debug!("pdftotext failed on file {:?}", path);
        return Ok(Vec::new());
    }

    let raw_text = String::from_utf8_lossy(&output.stdout);
    // Split on form feed character '\x0c'
    let pages: Vec<&str> = raw_text.split('\x0c').collect();

    let mut chunks = Vec::new();

    for (page_number, page_str) in (1..).zip(pages) {
        let trimmed = page_str.trim();
        if !trimmed.is_empty() {
            chunks.push(DocumentChunk {
                chunk_index: (page_number - 1) as u32,
                page_number: Some(page_number),
                line_number: None,
                content: trimmed.to_string(),
            });
        }
    }

    Ok(chunks)
}

/// Universal chunk extractor that delegates to the appropriate extractor based on file extension.
pub fn extract_document_chunks(path: &Path) -> Result<Vec<DocumentChunk>> {
    if !path.exists() {
        return Err(SiftError::PathNotFound(path.to_string_lossy().to_string()));
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "pdf" {
        extract_pdf_chunks(path, MAX_INDEXABLE_FILE_SIZE_BYTES)
    } else if is_text_or_code_extension(&ext) {
        extract_text_chunks(path, MAX_INDEXABLE_FILE_SIZE_BYTES)
    } else {
        // Not a text or PDF file (e.g. image, archive, binary)
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_extract_text_chunks_small() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Line 1: Hello Sift").unwrap();
        writeln!(file, "Line 2: Offline desktop search").unwrap();

        let chunks = extract_text_chunks(file.path(), 1024 * 1024).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk_index, 0);
        assert_eq!(chunks[0].line_number, Some(1));
        assert!(chunks[0].content.contains("Offline desktop search"));
    }

    #[test]
    fn test_extract_text_chunks_large_multichunk() {
        let mut file = NamedTempFile::new().unwrap();
        for i in 1..=120 {
            writeln!(file, "Line {}: code statement number {}", i, i).unwrap();
        }

        let chunks = extract_text_chunks(file.path(), 1024 * 1024).unwrap();
        assert!(chunks.len() >= 2);
        assert_eq!(chunks[0].line_number, Some(1));
        assert_eq!(chunks[1].line_number, Some(41)); // 50 - 10 + 1 = 41
    }

    #[test]
    fn test_skip_binary_file() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(&[0x7f, b'E', b'L', b'F', 0x00, 0x01])
            .unwrap();

        let chunks = extract_text_chunks(file.path(), 1024 * 1024).unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_extract_pdf_fixture() {
        let fixture_path = Path::new("tests/fixtures/documents/minimal_passage.pdf");
        if fixture_path.exists() {
            let chunks = extract_pdf_chunks(fixture_path, 1024 * 1024).unwrap();
            assert!(!chunks.is_empty());
            assert_eq!(chunks[0].page_number, Some(1));
            assert!(chunks[0].content.contains("Quantum indexing benchmark"));
        }
    }
}
