//! The game window: a Tauri app whose whole UI is one static HTML page.

/// Builds the Tauri app and runs it until the window closes.
///
/// # Errors
/// Returns the Tauri runtime error if the webview cannot be created.
pub fn run() -> tauri::Result<()> {
    tauri::Builder::default().run(tauri::generate_context!())
}
