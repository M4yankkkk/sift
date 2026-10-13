# Sift — Ubuntu Setup and Verification Guide

## 1. System Requirements

- **Operating System:** Ubuntu 24.04 LTS (x86_64 or aarch64)
- **Node.js:** v20+ or v24+
- **Rust Toolchain:** 1.80+ (tested on rustc 1.95.0)

---

## 2. Ubuntu Prerequisites Installation

Tauri 2 requires WebKitGTK and GTK 3 development headers to link native window bindings on Ubuntu:

```bash
sudo apt update && sudo apt install -y \
  build-essential \
  curl \
  wget \
  file \
  libssl-dev \
  librsvg2-dev \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev \
  libayatana-appindicator3-dev
```

---

## 3. Workspace Build & Test Commands

### Core Rust Engine (No GUI headers required)
The core domain logic, SQLite storage, scanner rules, models, and error boundary reside in `crates/sift-core`. This can be built and tested completely headlessly:

```bash
# Run unit tests
cargo test -p sift-core

# Run fixtures integration tests
cargo test -p sift-core --test fixtures_test

# Format and clippy lint checks
cargo fmt --all -- --check
cargo clippy -p sift-core -- -D warnings
```

### Frontend (React + TypeScript + Vitest)
```bash
# Install dependencies
npm install

# Type-check TypeScript
npm run check

# Run frontend tests
npm test

# Build frontend assets for production
npm run build
```

### Full Desktop Application (Tauri 2)
```bash
# Run in development mode with live reload
npm run tauri dev

# Build standalone Debian/AppImage desktop package
npm run tauri build
```

---

## 4. Production Installation

### Method A: One-Command Turnkey Installer (Recommended)
Run the automated installation script, which builds/installs the `.deb` package, verifies Tesseract OCR, sets up login autostart, and registers the GNOME Wayland shortcut (`Super+Space` / `Ctrl+Alt+Space`):

```bash
./scripts/install.sh
```

### Method B: Manual Native `.deb` Package Installation (Ubuntu / Debian)
1. Build the release package:
   ```bash
   npm run tauri build
   ```
2. Install via `apt` (which resolves dependencies like WebKitGTK and AppIndicator automatically):
   ```bash
   sudo apt install ./src-tauri/target/release/bundle/deb/sift_0.1.0_amd64.deb
   ```
3. Launch or toggle anytime from terminal or shortcut:
   ```bash
   sift --toggle
   ```

### Method C: Portable AppImage (Universal Linux)
If you don't have root privileges or are running on Arch, Fedora, or another distribution:
```bash
chmod +x ./src-tauri/target/release/bundle/appimage/sift_0.1.0_amd64.AppImage
./src-tauri/target/release/bundle/appimage/sift_0.1.0_amd64.AppImage
```
