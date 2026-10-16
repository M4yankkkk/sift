#!/usr/bin/env bash
# ==============================================================================
# Sift — Production Installation & Setup Script for Ubuntu / Debian
# ==============================================================================
set -euo pipefail

BOLD='\033[1m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}${BOLD}==> Installing Sift for Production...${NC}"

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEB_DIR="$PROJECT_DIR/target/release/bundle/deb"

# 1. Check if binary or deb exists; build if needed
DEB_FILE=$(find "$DEB_DIR" -name "*.deb" 2>/dev/null | head -n 1 || true)

if [ -z "$DEB_FILE" ]; then
    echo -e "${YELLOW}==> Building production release bundle (deb + appimage)...${NC}"
    cd "$PROJECT_DIR"
    npm run tauri build
    DEB_FILE=$(find "$DEB_DIR" -name "*.deb" 2>/dev/null | head -n 1 || true)
fi

if [ -n "$DEB_FILE" ]; then
    echo -e "${GREEN}==> Installing Debian package:${NC} $DEB_FILE"
    sudo apt install -y "$DEB_FILE"
else
    echo -e "${YELLOW}==> Deb package not found. Installing standalone binary to /usr/local/bin/sift...${NC}"
    RELEASE_BIN="$PROJECT_DIR/target/release/sift"
    [ -f "$RELEASE_BIN" ] || RELEASE_BIN="$PROJECT_DIR/target/release/sift-tauri"
    if [ ! -f "$RELEASE_BIN" ]; then
        cargo build --release --manifest-path "$PROJECT_DIR/src-tauri/Cargo.toml"
    fi
    sudo cp "$RELEASE_BIN" /usr/local/bin/sift
    sudo chmod +x /usr/local/bin/sift
fi

# 2. Ensure Tesseract OCR is available for screenshot search
if ! command -v tesseract &>/dev/null; then
    echo -e "${BLUE}==> Installing Tesseract OCR engine...${NC}"
    sudo apt install -y tesseract-ocr tesseract-ocr-eng
fi

# PDF text extraction (pdftotext) and page previews (pdftoppm)
if ! command -v pdftotext &>/dev/null || ! command -v pdftoppm &>/dev/null; then
    echo -e "${BLUE}==> Installing poppler-utils for PDF support...${NC}"
    sudo apt install -y poppler-utils
fi

# 3. Configure Login Autostart (~/.config/autostart/sift.desktop)
echo -e "${BLUE}==> Configuring autostart on user login...${NC}"
AUTOSTART_DIR="$HOME/.config/autostart"
mkdir -p "$AUTOSTART_DIR"

cat <<EOF > "$AUTOSTART_DIR/sift.desktop"
[Desktop Entry]
Type=Application
Name=Sift
Comment=Offline-First Fast Desktop Search
Exec=sift
Icon=sift
Terminal=false
Categories=Utility;DesktopUtility;
X-GNOME-Autostart-enabled=true
EOF

# 4. Configure GNOME shortcut if running on GNOME / Ubuntu desktop
if [ "${XDG_CURRENT_DESKTOP:-}" = "ubuntu:GNOME" ] || [ "${XDG_CURRENT_DESKTOP:-}" = "GNOME" ]; then
    echo -e "${BLUE}==> Registering GNOME Wayland shortcut (Super+Space / Ctrl+Alt+Space)...${NC}"
    gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/sift/ name "Sift" 2>/dev/null || true
    gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/sift/ command "sift --toggle" 2>/dev/null || true
    gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/sift/ binding "<Super>space" 2>/dev/null || true

    EXISTING_BINDINGS=$(gsettings get org.gnome.settings-daemon.plugins.media-keys custom-keybindings 2>/dev/null || echo "@as []")
    if [[ "$EXISTING_BINDINGS" != *"custom-keybindings/sift/"* ]]; then
        if [ "$EXISTING_BINDINGS" = "@as []" ] || [ "$EXISTING_BINDINGS" = "[]" ]; then
            gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings "['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/sift/']"
        else
            UPDATED=$(echo "$EXISTING_BINDINGS" | sed "s/]$/, '\/org\/gnome\/settings-daemon\/plugins\/media-keys\/custom-keybindings\/sift\/']/")
            gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings "$UPDATED"
        fi
    fi
fi

echo -e "\n${GREEN}${BOLD}✓ Sift installed successfully!${NC}"
echo -e "• Executable: $(command -v sift || echo '/usr/bin/sift')"
echo -e "• Database:   $HOME/.local/share/sift/sift.db"
echo -e "• Toggle:     Run '${BOLD}sift --toggle${NC}' or press ${BOLD}Super+Space${NC} / ${BOLD}Ctrl+Alt+Space${NC}"
echo -e "• Autostart:  Configured in $AUTOSTART_DIR/sift.desktop\n"
