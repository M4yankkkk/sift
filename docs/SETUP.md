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
