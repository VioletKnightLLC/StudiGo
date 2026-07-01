// GoPro Webcam Studio - Tauri application entry point
// This is a stub for Phase 1 - full implementation follows in later tasks

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}