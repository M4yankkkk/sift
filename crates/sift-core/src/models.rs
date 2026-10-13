use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MatchType {
    Filename,
    Keyword,
    Semantic,
    Ocr,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ResourceMode {
    #[default]
    Eco,
    Performance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootFolder {
    pub id: String,
    pub path: String,
    pub added_at: DateTime<Utc>,
    pub file_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatus {
    pub total_files_discovered: i64,
    pub files_indexed: i64,
    pub queue_length: i64,
    pub is_scanning: boolean_json::Bool,
    pub is_indexing: boolean_json::Bool,
    pub last_scan_completed_at: Option<DateTime<Utc>>,
    pub current_operation: Option<String>,
}

mod boolean_json {
    pub type Bool = bool;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub query: String,
    pub search_type: Option<String>,
    pub root_paths: Option<Vec<String>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub id: String,
    pub path: String,
    pub filename: String,
    pub extension: String,
    pub size_bytes: i64,
    pub modified_at: DateTime<Utc>,
    pub score: f32,
    pub match_type: MatchType,
    pub match_reasons: Vec<String>,
    pub snippet: Option<String>,
    pub page_number: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultItem>,
    pub total_matches: usize,
    pub execution_time_ms: f64,
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OcrBoundingBox {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OcrWordRegion {
    pub text: String,
    pub confidence: f32,
    pub bbox: OcrBoundingBox,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OcrResult {
    pub full_text: String,
    pub average_confidence: f32,
    pub regions: Vec<OcrWordRegion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewResponse {
    pub id: String,
    pub path: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub preview_content: String,
    pub snippet: Option<String>,
    pub page_number: Option<i32>,
    pub ocr_regions: Option<Vec<OcrWordRegion>>,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub global_shortcut: String,
    pub resource_mode: ResourceMode,
    pub ml_enabled: bool,
    pub ocr_enabled: bool,
    pub default_exclusions: Vec<String>,
    pub max_file_size_mb: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            global_shortcut: "Ctrl+Alt+Space".to_string(),
            resource_mode: ResourceMode::Eco,


            ml_enabled: true,
            ocr_enabled: true,
            default_exclusions: vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                "dist".to_string(),
                ".cache".to_string(),
                ".local".to_string(),
                "tmp".to_string(),
            ],
            max_file_size_mb: 25,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: String,
    pub root_id: String,
    pub path: String,
    pub filename: String,
    pub extension: String,
    pub size_bytes: i64,
    pub modified_at: DateTime<Utc>,
    pub fingerprint: String,
    pub indexed_at: Option<DateTime<Utc>>,
    pub is_deleted: bool,
}
