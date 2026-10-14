import { describe, it, expect } from "vitest";
import { SiftAPI, isTauriEnvironment } from "../../src/services/tauriClient";
import type { SearchQuery, AppSettings } from "../../src/types/contract";

describe("Sift Frontend Contract and Client", () => {
  it("detects Tauri environment state safely", () => {
    // In node/vitest, isTauriEnvironment() should return false gracefully
    expect(isTauriEnvironment()).toBe(false);
  });

  it("returns mock roots in browser/test mode", async () => {
    const roots = await SiftAPI.listRoots();
    expect(Array.isArray(roots)).toBe(true);
    expect(roots.length).toBeGreaterThan(0);
    expect(roots[0]).toHaveProperty("path");
  });

  it("returns index status with expected contract shape", async () => {
    const status = await SiftAPI.getIndexStatus();
    expect(status).toHaveProperty("total_files_discovered");
    expect(status).toHaveProperty("files_indexed");
    expect(status).toHaveProperty("queue_length");
    expect(status).toHaveProperty("is_indexing");
  });

  it("handles search queries in fallback mode", async () => {
    const query: SearchQuery = {
      query: "roadmap",
      search_type: "filename",
    };
    const response = await SiftAPI.searchFiles(query);
    expect(response.results.length).toBeGreaterThan(0);
    expect(response.query).toBe("roadmap");
    expect(response.results[0].filename).toContain("project_roadmap.md");
  });

  it("fetches settings matching AppSettings interface", async () => {
    const settings: AppSettings = await SiftAPI.getSettings();
    expect(settings.resource_mode).toBe("eco");
    expect(settings.default_exclusions).toContain(".git");
    expect(settings.default_exclusions).toContain("node_modules");
  });

  it("handles openResult with and without target page numbers safely", async () => {
    await expect(SiftAPI.openResult("/home/user/document.pdf")).resolves.not.toThrow();
    await expect(SiftAPI.openResult("/home/user/document.pdf", 14)).resolves.not.toThrow();
    await expect(SiftAPI.openResult("/home/user/document.pdf", null)).resolves.not.toThrow();
  });
});
