#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    {
        // On Linux under Wayland, force X11 backend (XWayland) identical to Ulauncher.
        // This allows wmctrl and window managers to directly raise and focus the window
        // without GNOME Mutter triggering the '"Sift" is ready' notification banner.
        std::env::set_var("GDK_BACKEND", "x11");
    }

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--toggle" || a == "-t") {
        if sift_lib::send_toggle_signal() {
            return;
        }
    }
    sift_lib::run();
}
