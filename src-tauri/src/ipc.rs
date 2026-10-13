use tauri::Manager;

pub fn toggle_main_window(app: &tauri::AppHandle) -> bool {
    if let Some(window) = app.get_webview_window("main") {
        let is_visible = window.is_visible().unwrap_or(false);
        let is_focused = window.is_focused().unwrap_or(false);
        if is_visible && is_focused {
            let _ = window.hide();
            return false;
        } else {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_always_on_top(true);
            let _ = window.set_focus();
            return true;
        }
    }
    false
}

pub fn send_toggle_signal() -> bool {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::net::UnixStream;

        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
        let sock_path = std::path::PathBuf::from(runtime_dir).join("sift.sock");
        if let Ok(mut stream) = UnixStream::connect(&sock_path) {
            let _ = stream.write_all(b"toggle\n");
            return true;
        }

        let user = std::env::var("USER").unwrap_or_else(|_| "default".to_string());
        let fallback_path = std::path::PathBuf::from(format!("/tmp/sift-{}.sock", user));
        if let Ok(mut stream) = UnixStream::connect(&fallback_path) {
            let _ = stream.write_all(b"toggle\n");
            return true;
        }
    }
    false
}

#[cfg(unix)]
pub fn start_ipc_server(app_handle: tauri::AppHandle) {
    use std::io::Read;
    use std::os::unix::net::UnixListener;

    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    let mut sock_path = std::path::PathBuf::from(runtime_dir).join("sift.sock");

    let _ = std::fs::remove_file(&sock_path);
    let listener = match UnixListener::bind(&sock_path) {
        Ok(l) => l,
        Err(_) => {
            let user = std::env::var("USER").unwrap_or_else(|_| "default".to_string());
            sock_path = std::path::PathBuf::from(format!("/tmp/sift-{}.sock", user));
            let _ = std::fs::remove_file(&sock_path);
            match UnixListener::bind(&sock_path) {
                Ok(l) => l,
                Err(e) => {
                    tracing::warn!("Failed to bind Sift IPC socket at {:?}: {}", sock_path, e);
                    return;
                }
            }
        }
    };

    tracing::info!("Sift IPC listener running on {:?}", sock_path);
    for stream in listener.incoming() {
        if let Ok(mut stream) = stream {
            let mut buf = [0u8; 16];
            let _ = stream.read(&mut buf);
            let shown = toggle_main_window(&app_handle);
            use std::io::Write;
            if shown {
                let _ = stream.write_all(b"shown\n");
            } else {
                let _ = stream.write_all(b"hidden\n");
            }
        }
    }
}

pub fn format_gnome_binding(shortcut: &str) -> String {
    let mut parts = Vec::new();
    let lower = shortcut.to_lowercase();
    if lower.contains("ctrl") || lower.contains("control") {
        parts.push("<Control>");
    }
    if lower.contains("alt") {
        parts.push("<Alt>");
    }
    if lower.contains("shift") {
        parts.push("<Shift>");
    }
    if lower.contains("super") || lower.contains("cmd") || lower.contains("win") {
        parts.push("<Super>");
    }

    let key = shortcut
        .split('+')
        .last()
        .unwrap_or("space")
        .trim()
        .to_lowercase();
    format!("{}{}", parts.concat(), key)
}

pub fn sync_gnome_shortcut(shortcut: &str) {
    if std::env::var("XDG_CURRENT_DESKTOP")
        .map(|d| d.to_lowercase().contains("gnome"))
        .unwrap_or(false)
    {
        let binding = format_gnome_binding(shortcut);
        let script_path = if let Ok(exe) = std::env::current_exe() {
            format!("{} --toggle", exe.to_string_lossy())
        } else {
            "sift --toggle".to_string()
        };

        let _ = std::process::Command::new("gsettings")
            .args([
                "set",
                "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom1/",
                "name",
                "Sift",
            ])
            .output();

        let _ = std::process::Command::new("gsettings")
            .args([
                "set",
                "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom1/",
                "command",
                &script_path,
            ])
            .output();

        let _ = std::process::Command::new("gsettings")
            .args([
                "set",
                "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom1/",
                "binding",
                &binding,
            ])
            .output();

        if let Ok(output) = std::process::Command::new("gsettings")
            .args([
                "get",
                "org.gnome.settings-daemon.plugins.media-keys",
                "custom-keybindings",
            ])
            .output()
        {
            let s = String::from_utf8_lossy(&output.stdout);
            if !s.contains("custom1") {
                let new_list = if s.contains("custom0") {
                    "['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/', '/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom1/']"
                } else {
                    "['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom1/']"
                };
                let _ = std::process::Command::new("gsettings")
                    .args([
                        "set",
                        "org.gnome.settings-daemon.plugins.media-keys",
                        "custom-keybindings",
                        new_list,
                    ])
                    .output();
            }
        }
        tracing::info!("Synced GNOME shortcut binding to '{}'", binding);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_gnome_binding() {
        assert_eq!(format_gnome_binding("Ctrl+Alt+Space"), "<Control><Alt>space");
        assert_eq!(format_gnome_binding("Alt+Shift+S"), "<Alt><Shift>s");
        assert_eq!(format_gnome_binding("Super+Space"), "<Super>space");
    }
}
