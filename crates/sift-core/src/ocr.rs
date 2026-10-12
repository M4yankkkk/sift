use crate::error::{Result, SiftError};
use crate::models::{OcrBoundingBox, OcrResult, OcrWordRegion};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Maximum image file size eligible for OCR extraction (15 MB).
pub const MAX_OCR_IMAGE_SIZE_BYTES: u64 = 15 * 1024 * 1024;

/// Supported image extensions for OCR text recognition.
pub fn is_ocr_supported_extension(ext: &str) -> bool {
    matches!(
        ext.to_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "bmp"
    )
}

fn user_home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
}

/// Locates the tesseract executable on the system PATH or common local fallback locations.
pub fn find_tesseract_binary() -> Option<PathBuf> {
    // 1. Check if tesseract is callable via PATH
    if let Ok(output) = Command::new("tesseract").arg("--version").output() {
        if output.status.success() {
            return Some(PathBuf::from("tesseract"));
        }
    }

    // 2. Check ~/.local/bin/tesseract
    if let Some(home) = user_home_dir() {
        let local_bin = home.join(".local").join("bin").join("tesseract");
        if local_bin.exists() {
            return Some(local_bin);
        }
    }

    // 3. Check system standard paths
    let system_paths = ["/usr/bin/tesseract", "/usr/local/bin/tesseract"];
    for p in system_paths {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// Checks whether OCR extraction is functional on this machine.
pub fn is_ocr_available() -> bool {
    find_tesseract_binary().is_some()
}

/// Extracts text and word bounding boxes from an image file using Tesseract OCR.
pub fn extract_image_ocr(image_path: &Path) -> Result<Option<OcrResult>> {
    if !image_path.exists() {
        return Err(SiftError::PathNotFound(
            image_path.to_string_lossy().to_string(),
        ));
    }

    let ext = image_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if !is_ocr_supported_extension(&ext) {
        return Ok(None);
    }

    // File size safety check
    let metadata = std::fs::metadata(image_path).map_err(SiftError::Io)?;
    if metadata.len() > MAX_OCR_IMAGE_SIZE_BYTES || metadata.len() == 0 {
        tracing::warn!(
            path = ?image_path,
            size = metadata.len(),
            "Skipping OCR for image exceeding size limit or zero bytes"
        );
        return Ok(None);
    }

    let tesseract_bin = match find_tesseract_binary() {
        Some(b) => b,
        None => {
            tracing::debug!("Tesseract binary not found on system; skipping OCR extraction");
            return Ok(None);
        }
    };

    let mut cmd = Command::new(&tesseract_bin);
    cmd.arg(image_path);
    cmd.arg("stdout");
    cmd.arg("-l");
    cmd.arg("eng");
    cmd.arg("tsv");

    // Configure fallback environment variables if local user-space install is used
    if let Some(home) = user_home_dir() {
        let tessdata = home
            .join(".local")
            .join("usr")
            .join("share")
            .join("tesseract-ocr")
            .join("5")
            .join("tessdata");
        if tessdata.exists() {
            cmd.env("TESSDATA_PREFIX", tessdata);
        }

        let libdir = home
            .join(".local")
            .join("usr")
            .join("lib")
            .join("x86_64-linux-gnu");
        if libdir.exists() {
            let current_ld = std::env::var("LD_LIBRARY_PATH").unwrap_or_default();
            cmd.env(
                "LD_LIBRARY_PATH",
                format!("{}:{current_ld}", libdir.display()),
            );
        }
    }

    let output = match cmd.output() {
        Ok(out) => out,
        Err(e) => {
            tracing::warn!(
                error = ?e,
                path = ?image_path,
                "Failed to execute tesseract process for image"
            );
            return Ok(None);
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(
            path = ?image_path,
            status = ?output.status,
            stderr = %stderr,
            "Tesseract process returned non-zero status for image"
        );
        return Ok(None);
    }

    let tsv_text = String::from_utf8_lossy(&output.stdout);
    parse_tesseract_tsv(&tsv_text)
}

/// Parses Tesseract TSV output into an OcrResult with word bounding boxes.
pub fn parse_tesseract_tsv(tsv_content: &str) -> Result<Option<OcrResult>> {
    let mut words = Vec::new();
    let mut regions = Vec::new();
    let mut conf_sum = 0.0f32;
    let mut conf_count = 0usize;

    for line in tsv_content.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        // TSV columns: level, page_num, block_num, par_num, line_num, word_num, left, top, width, height, conf, text
        if cols.len() < 12 {
            continue;
        }

        let level = cols[0].trim();
        if level != "5" {
            // Level 5 corresponds to word-level entries
            continue;
        }

        let text = cols[11].trim();
        if text.is_empty() {
            continue;
        }

        let conf: f32 = cols[10].trim().parse().unwrap_or(-1.0);
        if conf < 0.0 {
            continue;
        }

        let left: i32 = cols[6].trim().parse().unwrap_or(0);
        let top: i32 = cols[7].trim().parse().unwrap_or(0);
        let width: i32 = cols[8].trim().parse().unwrap_or(0);
        let height: i32 = cols[9].trim().parse().unwrap_or(0);

        words.push(text.to_string());
        conf_sum += conf;
        conf_count += 1;

        regions.push(OcrWordRegion {
            text: text.to_string(),
            confidence: conf,
            bbox: OcrBoundingBox {
                left,
                top,
                width,
                height,
            },
        });
    }

    if regions.is_empty() {
        return Ok(None);
    }

    let average_confidence = if conf_count > 0 {
        conf_sum / (conf_count as f32)
    } else {
        0.0
    };

    let full_text = words.join(" ");

    Ok(Some(OcrResult {
        full_text,
        average_confidence,
        regions,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sample_tsv() {
        let sample = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
1\t1\t0\t0\t0\t0\t0\t0\t400\t100\t-1\t\n\
5\t1\t1\t1\t1\t1\t21\t32\t22\t8\t85.5\tError\n\
5\t1\t1\t1\t1\t2\t46\t32\t17\t8\t92.0\t404\n\
5\t1\t1\t1\t1\t3\t68\t32\t41\t8\t88.5\tNot\n\
5\t1\t1\t1\t1\t4\t113\t32\t15\t8\t90.0\tFound\n";

        let result = parse_tesseract_tsv(sample).unwrap().expect("should parse");
        assert_eq!(result.full_text, "Error 404 Not Found");
        assert_eq!(result.regions.len(), 4);
        assert_eq!(result.regions[0].text, "Error");
        assert_eq!(result.regions[0].bbox.left, 21);
        assert_eq!(result.regions[1].text, "404");
        assert!(result.average_confidence > 88.0);
    }

    #[test]
    fn test_is_ocr_supported_extension() {
        assert!(is_ocr_supported_extension("png"));
        assert!(is_ocr_supported_extension("PNG"));
        assert!(is_ocr_supported_extension("jpg"));
        assert!(is_ocr_supported_extension("jpeg"));
        assert!(is_ocr_supported_extension("webp"));
        assert!(!is_ocr_supported_extension("pdf"));
        assert!(!is_ocr_supported_extension("txt"));
    }
}
