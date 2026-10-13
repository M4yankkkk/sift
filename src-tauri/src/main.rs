#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--toggle" || a == "-t") {
        if sift_lib::send_toggle_signal() {
            return;
        }
    }
    sift_lib::run();
}
