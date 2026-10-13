import { invoke } from "@tauri-apps/api/core";
import type {
  RootFolder,
  IndexStatus,
  SearchQuery,
  SearchResponse,
  PreviewResponse,
  AppSettings,
} from "../types/contract";

// Detection helper for Tauri desktop environment
export const isTauriEnvironment = (): boolean => {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
};

export const SiftAPI = {
  // Roots
  listRoots: async (): Promise<RootFolder[]> => {
    if (!isTauriEnvironment()) {
      return [
        {
          id: "mock-1",
          path: "/home/user/Documents",
          added_at: new Date().toISOString(),
          file_count: 142,
        },
      ];
    }
    return invoke<RootFolder[]>("list_roots");
  },

  addRoot: async (path: string): Promise<RootFolder> => {
    if (!isTauriEnvironment()) {
      return {
        id: "mock-new",
        path,
        added_at: new Date().toISOString(),
        file_count: 0,
      };
    }
    return invoke<RootFolder>("add_root", { path });
  },

  removeRoot: async (path: string): Promise<boolean> => {
    if (!isTauriEnvironment()) return true;
    return invoke<boolean>("remove_root", { path });
  },

  // Indexing & Scanning
  startScan: async (): Promise<IndexStatus> => {
    if (!isTauriEnvironment()) {
      return {
        total_files_discovered: 142,
        files_indexed: 142,
        queue_length: 0,
        is_scanning: false,
        is_indexing: false,
        last_scan_completed_at: new Date().toISOString(),
        current_operation: "Idle (mock mode)",
      };
    }
    return invoke<IndexStatus>("start_scan");
  },

  reindexAll: async (): Promise<IndexStatus> => {
    if (!isTauriEnvironment()) {
      return {
        total_files_discovered: 142,
        files_indexed: 142,
        queue_length: 0,
        is_scanning: false,
        is_indexing: false,
        last_scan_completed_at: new Date().toISOString(),
        current_operation: "Idle (mock mode)",
      };
    }
    return invoke<IndexStatus>("reindex_all");
  },

  getIndexStatus: async (): Promise<IndexStatus> => {
    if (!isTauriEnvironment()) {
      return {
        total_files_discovered: 142,
        files_indexed: 142,
        queue_length: 0,
        is_scanning: false,
        is_indexing: false,
        last_scan_completed_at: new Date().toISOString(),
        current_operation: "Idle (browser preview)",
      };
    }
    return invoke<IndexStatus>("get_index_status");
  },

  // Search & Actions
  searchFiles: async (query: SearchQuery): Promise<SearchResponse> => {
    if (!isTauriEnvironment()) {
      return {
        results: [
          {
            id: "mock-res-1",
            path: "/home/user/Documents/project_roadmap.md",
            filename: "project_roadmap.md",
            extension: "md",
            size_bytes: 4096,
            modified_at: new Date().toISOString(),
            score: 0.95,
            match_type: "filename",
            match_reasons: ["Matched filename 'project_roadmap.md'"],
            snippet: "Phase 0: Tauri 2 + Rust + React setup complete.",
            page_number: null,
          },
        ],
        total_matches: 1,
        execution_time_ms: 1.2,
        query: query.query,
      };
    }
    return invoke<SearchResponse>("search_files", { query });
  },

  previewResult: async (fileId: string, page?: number | null): Promise<PreviewResponse> => {
    if (!isTauriEnvironment()) {
      return {
        id: fileId,
        path: "/home/user/Documents/project_roadmap.md",
        filename: "project_roadmap.md",
        mime_type: "text/markdown",
        size_bytes: 4096,
        preview_content: "# Project Roadmap\n\nPhase 0 completed successfully.",
        snippet: null,
        page_number: page || null,
      };
    }
    return invoke<PreviewResponse>("preview_result", { fileId, page: page || null });
  },

  openResult: async (path: string): Promise<void> => {
    if (!isTauriEnvironment()) {
      console.log("[Mock] Opening file:", path);
      return;
    }
    return invoke<void>("open_result", { path });
  },

  revealResult: async (path: string): Promise<void> => {
    if (!isTauriEnvironment()) {
      console.log("[Mock] Revealing file in file manager:", path);
      return;
    }
    return invoke<void>("reveal_result", { path });
  },

  // Settings
  getSettings: async (): Promise<AppSettings> => {
    if (!isTauriEnvironment()) {
      return {
        global_shortcut: "Ctrl+Space",
        resource_mode: "eco",

        ml_enabled: false,
        ocr_enabled: false,
        default_exclusions: [
          ".git",
          "node_modules",
          "target",
          "dist",
          ".cache",
        ],
        max_file_size_mb: 25,
      };
    }
    return invoke<AppSettings>("get_settings");
  },

  setSettings: async (settings: AppSettings): Promise<AppSettings> => {
    if (!isTauriEnvironment()) return settings;
    return invoke<AppSettings>("set_settings", { settings });
  },

  hideWindow: async (): Promise<void> => {
    if (!isTauriEnvironment()) return;
    return invoke<void>("hide_window");
  },
};

