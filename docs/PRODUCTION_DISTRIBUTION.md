# Sift — Production Packaging & Distribution Architecture

This document specifies the distribution model, installer design, and first-run onboarding architecture for **Sift** on Linux (Ubuntu / Debian, Arch, Fedora).

---

## 1. Problem Statement & Architecture Goals

Desktop applications integrating on-device neural models face a packaging dilemma:
* **Fat Packages (~600 MB – 1.2 GB):** Bundling the compiled binary, WebKit runtime, Tesseract OCR, and EmbeddingGemma 2 ONNX weights (740M fp32 / 270M int8) produces massive downloads. Every minor bug fix or UI patch forces users to re-download hundreds of megabytes.
* **Complex Developer Setups:** Requiring users to install Python, Hugging Face CLI, or compile from source is unacceptable for an everyday desktop utility like Alfred or Raycast.

**Design Goal:** Deliver an **Android-like, frictionless experience** where the initial download is tiny and fast, system dependencies install automatically via standard OS package managers, and neural models are downloaded and cached once on first launch.

---

## 2. The Recommended Pattern: "Thin Client + First-Run Setup"

Sift adopts the **Thin Client + First-Run Setup** pattern used by industry benchmarks such as **Steam, LM Studio, Ollama, and Discord**.

```
┌────────────────────────────────┐
│  Download Sift.deb (~15 MB)    │  <-- Fast 2-second download
└───────────────┬────────────────┘
                │
                ▼
┌────────────────────────────────┐
│  sudo apt install ./sift.deb   │  <-- APT automatically resolves:
└───────────────┬────────────────┘      - libwebkit2gtk-4.1-0
                │                       - libayatana-appindicator3-1
                │                       - tesseract-ocr (System OCR engine)
                ▼
┌────────────────────────────────┐
│  First Launch: Onboarding UI   │  <-- "Initializing Local Neural Engine"
│  [████████████░░░░░] 70%       │      - Downloads EmbeddingGemma 2 once
└───────────────┬────────────────┘      - Caches in ~/.cache/fastembed/
                │
                ▼
┌────────────────────────────────┐
│  Ready for Instant Offline Use │  <-- 100% offline from this point forward
└────────────────────────────────┘
```

### Key Advantages:
1. **Tiny Initial Download:** The `.deb` package remains **~15–20 MB** (containing the Rust backend, React frontend, icons, and desktop entries).
2. **Bandwidth-Friendly Updates:** Updating Sift from `v0.1.0` to `v0.2.0` only requires downloading a fresh 15 MB binary; the cached ~500 MB model is never re-downloaded.
3. **Automatic System Dependencies:** Declaring `Depends: tesseract-ocr` in the `.deb` control file ensures Ubuntu's package manager handles OCR installation automatically with zero user configuration.
4. **Graceful Offline Degradation:** If launched without internet on first run, Sift immediately operates in **Lexical Mode** (filename search + Tantivy BM25 full-text search) and downloads the neural model when connectivity is restored.

---

## 3. Distribution Options Compared

| Feature | Thin `.deb` (Recommended) | Pre-Bundled "Fat" `.deb` | Portable `AppImage` | Flatpak (Flathub) |
|---|---|---|---|---|
| **Installer Size** | **~15–20 MB** | ~650–900 MB | ~700–950 MB | ~100 MB + runtime |
| **First-Run Time** | ~30s (model stream) | Instant | Instant | ~1 min (runtime sync) |
| **App Update Size** | **~15 MB** | ~650–900 MB | ~700–950 MB | ~20 MB (diff-based) |
| **Root Required?** | Yes (`apt install`) | Yes (`apt install`) | **No (Zero-Root)** | No / User flatpak |
| **System OCR Integration** | Auto (`apt depends`) | Auto (`apt depends`) | Bundled inside image | Bundled in sandbox |
| **Desktop / Hotkey Hook** | Native GNOME integration | Native GNOME integration | Manual or `appimaged` | XDG Desktop Portal |
| **Target Audience** | Ubuntu / Debian users | Enterprise / Air-gapped | Portable / Any Linux | Modern GNOME / Fedora |

---

## 4. Production Installation Workflows

### 4.1 Turnkey 1-Command Installer
The repository includes [`scripts/install.sh`](file:///home/mayank-tiwari/Projects/sift/scripts/install.sh) which automates the end-to-end setup:
```bash
./scripts/install.sh
```
* Builds the production package (`npm run tauri build`).
* Installs the `.deb` via `apt` (resolving WebKit and Tesseract).
* Adds the login autostart entry (`~/.config/autostart/sift.desktop`).
* Registers the native GNOME Wayland shortcut (`Super+Space` / `Ctrl+Alt+Space`) to `sift --toggle`.

### 4.2 Standard Manual Debian Installation
```bash
# 1. Compile release binary and bundle
npm run tauri build

# 2. Install package
sudo apt install ./src-tauri/target/release/bundle/deb/sift_0.1.0_amd64.deb

# 3. Summon or toggle anytime
sift --toggle
```

### 4.3 Portable Universal AppImage
```bash
chmod +x ./src-tauri/target/release/bundle/appimage/sift_0.1.0_amd64.AppImage
./src-tauri/target/release/bundle/appimage/sift_0.1.0_amd64.AppImage
```

---

## 5. Storage & XDG Conformance

In production, Sift adheres strictly to the **XDG Base Directory Specification**:
* **Database & Indices:** `~/.local/share/sift/sift.db` and `~/.local/share/sift/tantivy_index/`
* **Model Cache:** `~/.cache/fastembed/` (or `~/.cache/sift/models/`)
* **Socket Daemon:** `$XDG_RUNTIME_DIR/sift.sock` (typically `/run/user/$UID/sift.sock`)
* **Autostart:** `~/.config/autostart/sift.desktop`
