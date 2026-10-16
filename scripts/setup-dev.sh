#!/usr/bin/env bash
# Installs everything needed to build and run Sift from source on Ubuntu/Debian:
# GTK/WebKit development libraries for the Tauri window, plus Tesseract (OCR) and poppler (PDFs).
set -euo pipefail

sudo apt-get update
sudo apt-get install -y \
  build-essential pkg-config libssl-dev \
  libgtk-3-dev libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev \
  libayatana-appindicator3-dev librsvg2-dev \
  tesseract-ocr tesseract-ocr-eng poppler-utils

echo "Done. Now run: npm install && npm run tauri dev"
