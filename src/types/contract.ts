/**
 * Typed Tauri Command and Event Contract for Sift
 * Defines all shared data structures between Rust backend and React frontend.
 */

export type MatchType = "filename" | "keyword" | "semantic" | "ocr";
export type ResourceMode = "eco" | "performance";

export interface RootFolder {
  id: string;
  path: string;
  added_at: string; // ISO 8601
  file_count: number;
}

export interface IndexStatus {
  total_files_discovered: number;
  files_indexed: number;
  queue_length: number;
  is_scanning: boolean;
  is_indexing: boolean;
  last_scan_completed_at: string | null;
  current_operation: string | null;
}

export interface SearchQuery {
  query: string;
  search_type?: "filename" | "content" | "hybrid";
  root_paths?: string[];
  limit?: number;
  offset?: number;
}

export interface SearchResultItem {
  id: string;
  path: string;
  filename: string;
  extension: string;
  size_bytes: number;
  modified_at: string;
  score: number;
  match_type: MatchType;
  match_reasons: string[];
  snippet: string | null;
  page_number: number | null;
}

export interface SearchResponse {
  results: SearchResultItem[];
  total_matches: number;
  execution_time_ms: number;
  query: string;
}

export interface OcrBoundingBox {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface OcrWordRegion {
  text: string;
  confidence: number;
  bbox: OcrBoundingBox;
}

export interface PreviewResponse {
  id: string;
  path: string;
  filename: string;
  mime_type: string;
  size_bytes: number;
  preview_content: string;
  snippet: string | null;
  page_number: number | null;
  ocr_regions?: OcrWordRegion[] | null;
  confidence?: number | null;
}

export interface AppSettings {
  global_shortcut: string;
  resource_mode: ResourceMode;
  ml_enabled: boolean;
  ocr_enabled: boolean;
  default_exclusions: string[];
  max_file_size_mb: number;
}

export interface AppError {
  code: string;
  message: string;
  details?: string;
}

export type TauriResponse<T> =
  | { success: true; data: T }
  | { success: false; error: AppError };
