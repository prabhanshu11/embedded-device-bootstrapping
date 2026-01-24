//! pibox-core: Shared library for embedded device orchestration
//!
//! This crate provides:
//! - Protocol types for WebSocket communication
//! - JWT authentication
//! - Application state machine with undo/redo
//! - Filebrowser REST client

pub mod auth;
pub mod config;
pub mod filebrowser;
pub mod protocol;
pub mod state;

pub use auth::{Claims, JwtAuth, TokenPair};
pub use config::Config;
pub use filebrowser::FilebrowserClient;
pub use protocol::{ClientMessage, ServerMessage};
pub use state::{AppState, FileEntry, FileType};

/// Default WebSocket port for pibox-server
pub const DEFAULT_WS_PORT: u16 = 9280;

/// Default Filebrowser backend port (localhost only)
pub const DEFAULT_FILEBROWSER_PORT: u16 = 8080;
