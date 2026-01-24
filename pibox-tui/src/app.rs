//! Application state and logic

use pibox_core::{
    state::{AppState, FileEntry, FileType, StatusLevel},
    Config,
};

/// Application result for main loop
pub enum AppResult {
    Continue,
    Quit,
}

/// Main application struct
pub struct App {
    /// Configuration
    pub config: Config,

    /// UI state
    pub state: AppState,

    /// Connection status message
    pub status_text: String,

    /// Whether we're connected to server
    pub connected: bool,
}

impl App {
    pub fn new(config: Config) -> Self {
        // Get default server URL from config
        let server_url = config
            .client
            .default_device
            .as_ref()
            .and_then(|name| config.get_device(name))
            .map(|d| d.url.clone())
            .unwrap_or_else(|| format!("ws://localhost:{}", pibox_core::DEFAULT_WS_PORT));

        let state = AppState::new(&server_url);

        // Start with demo data for now
        let mut app = Self {
            config,
            state,
            status_text: "Not connected".to_string(),
            connected: false,
        };

        // Load demo data
        app.load_demo_data();

        app
    }

    /// Load demo data for testing without server
    fn load_demo_data(&mut self) {
        let entries = vec![
            FileEntry {
                name: "Documents".to_string(),
                path: "/Documents".to_string(),
                file_type: FileType::Directory,
                size: 0,
                modified: 1706000000,
                mime_type: None,
            },
            FileEntry {
                name: "Downloads".to_string(),
                path: "/Downloads".to_string(),
                file_type: FileType::Directory,
                size: 0,
                modified: 1706100000,
                mime_type: None,
            },
            FileEntry {
                name: "Movies".to_string(),
                path: "/Movies".to_string(),
                file_type: FileType::Directory,
                size: 0,
                modified: 1705900000,
                mime_type: None,
            },
            FileEntry {
                name: "Music".to_string(),
                path: "/Music".to_string(),
                file_type: FileType::Directory,
                size: 0,
                modified: 1705800000,
                mime_type: None,
            },
            FileEntry {
                name: "readme.txt".to_string(),
                path: "/readme.txt".to_string(),
                file_type: FileType::File,
                size: 1234,
                modified: 1706200000,
                mime_type: Some("text/plain".to_string()),
            },
            FileEntry {
                name: "photo.jpg".to_string(),
                path: "/photo.jpg".to_string(),
                file_type: FileType::File,
                size: 2_500_000,
                modified: 1706150000,
                mime_type: Some("image/jpeg".to_string()),
            },
            FileEntry {
                name: "video.mp4".to_string(),
                path: "/video.mp4".to_string(),
                file_type: FileType::File,
                size: 150_000_000,
                modified: 1706050000,
                mime_type: Some("video/mp4".to_string()),
            },
        ];

        self.state.set_entries("/".to_string(), entries);
        self.status_text = "Demo mode (no server connection)".to_string();
    }

    /// Process async operations
    pub async fn tick(&mut self) {
        // TODO: Process WebSocket messages, update state
    }

    /// Navigate to a directory
    pub async fn navigate_to(&mut self, path: &str) {
        // TODO: Request directory listing from server
        self.state.set_status(format!("Navigate to: {}", path), StatusLevel::Info);
    }

    /// Go up one directory
    pub async fn navigate_up(&mut self) {
        if let Some(parent) = self.state.parent_path() {
            self.navigate_to(&parent).await;
        }
    }

    /// Enter selected directory or open file
    pub async fn enter(&mut self) {
        if let Some(entry) = self.state.current_entry() {
            if entry.is_dir() {
                let path = entry.path.clone();
                self.navigate_to(&path).await;
            } else {
                // TODO: Open file
                self.state.set_status(format!("Open: {}", entry.name), StatusLevel::Info);
            }
        }
    }

    /// Delete selected entries
    pub async fn delete_selected(&mut self) {
        let paths = self.state.selected_paths();
        if paths.is_empty() {
            return;
        }

        // TODO: Confirm and delete
        self.state.set_status(
            format!("Delete {} item(s)?", paths.len()),
            StatusLevel::Warning,
        );
    }

    /// Copy selected entries to clipboard
    pub fn copy_selected(&mut self) {
        let paths = self.state.selected_paths();
        self.state.set_status(
            format!("Copied {} item(s)", paths.len()),
            StatusLevel::Success,
        );
    }

    /// Paste from clipboard
    pub async fn paste(&mut self) {
        // TODO: Implement paste
        self.state.set_status("Paste (not implemented)", StatusLevel::Info);
    }
}
