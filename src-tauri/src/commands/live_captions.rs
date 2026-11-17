use std::path::Path;
use tauri::{command, AppHandle};

// Async so the engine load and the worker join never block the main thread.
#[command]
pub async fn start_live_captions(app: AppHandle, model: Option<String>) -> Result<(), String> {
    crate::live_captions::start(&app, model)
}

#[command]
pub async fn stop_live_captions(app: AppHandle) {
    crate::live_captions::stop(&app);
}

#[command]
pub fn is_live_captions_running(app: AppHandle) -> bool {
    crate::live_captions::is_running(&app)
}

/// Writes the captions where the user chose to save them. Text files only:
/// the webview must not be able to overwrite any other kind of file.
#[command]
pub fn export_live_captions(file_path: String, content: String) -> Result<(), String> {
    let is_text_file = Path::new(&file_path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("txt"));
    match is_text_file {
        true => std::fs::write(&file_path, content).map_err(|e| e.to_string()),
        false => Err("File must have a .txt extension".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_writes_text_files_only() {
        let folder = std::env::temp_dir();
        let content = "Hello everyone.\n".to_string();

        let text_file = folder.join("murmure-live-captions-export-test.txt");
        let written = export_live_captions(text_file.to_string_lossy().into(), content.clone());
        assert!(written.is_ok());
        assert_eq!(std::fs::read_to_string(&text_file).unwrap(), content);
        let _ = std::fs::remove_file(&text_file);

        let other_file = folder.join("murmure-live-captions-export-test.exe");
        let refused = export_live_captions(other_file.to_string_lossy().into(), content);
        assert!(refused.is_err());
        assert!(!other_file.exists());
    }
}
