//! WebSocket protocol types for pibox communication
//!
//! All messages are JSON-serialized. The protocol supports:
//! - Authentication (login, token refresh)
//! - File operations (list, download, upload, delete, rename)
//! - Load monitoring (adaptive offloading)
//! - Real-time events (file changes, operation completion)

use serde::{Deserialize, Serialize};

/// Messages sent from client to server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Authenticate with username/password
    Login { username: String, password: String },

    /// Refresh access token using refresh token
    RefreshToken { refresh_token: String },

    /// List directory contents
    ListDir { path: String },

    /// Download file (server sends FileContent response)
    Download { path: String },

    /// Upload file
    Upload {
        path: String,
        #[serde(with = "base64_bytes")]
        content: Vec<u8>,
    },

    /// Delete file or directory
    Delete { path: String },

    /// Rename/move file or directory
    Rename { from: String, to: String },

    /// Create directory
    Mkdir { path: String },

    /// Report client capabilities (for adaptive offloading)
    Capabilities(ClientCapabilities),

    /// Response to offload request
    OffloadResult {
        task_id: String,
        #[serde(with = "base64_bytes")]
        result: Vec<u8>,
    },

    /// Ping for keepalive
    Ping,
}

/// Messages sent from server to client
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Authentication successful
    AuthSuccess(TokenPairResponse),

    /// Authentication failed
    AuthError { message: String },

    /// Directory listing
    DirListing {
        path: String,
        entries: Vec<FileEntryResponse>,
    },

    /// File content (for download)
    FileContent {
        path: String,
        #[serde(with = "base64_bytes")]
        content: Vec<u8>,
        mime_type: Option<String>,
    },

    /// Operation completed successfully
    OpSuccess { op: String, path: String },

    /// Operation failed
    OpError { op: String, path: String, message: String },

    /// Server load report (for adaptive behavior)
    Load(ServerLoad),

    /// Request client to handle a task (offloading)
    OffloadRequest {
        task_id: String,
        task: OffloadTask,
    },

    /// File system event (real-time sync)
    FsEvent(FsEvent),

    /// Pong response to ping
    Pong,

    /// Generic error
    Error { message: String },
}

/// Token pair for authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPairResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

/// File entry in directory listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntryResponse {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: i64, // Unix timestamp
    pub mime_type: Option<String>,
}

/// Server resource load for adaptive behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerLoad {
    pub cpu_percent: f32,
    pub ram_free_mb: u64,
    pub io_busy: bool,
    /// Suggested actions based on load
    pub hints: Vec<LoadHint>,
}

/// Hints for client behavior based on server load
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadHint {
    /// Reduce concurrent transfers
    ThrottleTransfers,
    /// Client should generate thumbnails locally
    GenerateThumbnailsLocally,
    /// Client should handle search locally
    SearchLocally,
    /// Server is recovering, operations may be slow
    Recovering,
}

/// Client capabilities for offload decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCapabilities {
    pub cpu_cores: u32,
    pub has_gpu: bool,
    pub ram_free_mb: u64,
    pub on_ac_power: bool,
    /// Features the client can handle
    pub can_generate_thumbnails: bool,
    pub can_search_locally: bool,
    pub can_compress: bool,
}

/// Task to offload to capable client
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "task_type", rename_all = "snake_case")]
pub enum OffloadTask {
    /// Generate thumbnail for image/video
    Thumbnail {
        path: String,
        #[serde(with = "base64_bytes")]
        source: Vec<u8>,
        width: u32,
        height: u32,
    },
    /// Search for text in files
    Search {
        query: String,
        paths: Vec<String>,
    },
}

/// File system event for real-time sync
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum FsEvent {
    Created { path: String, is_dir: bool },
    Modified { path: String },
    Deleted { path: String },
    Renamed { from: String, to: String },
}

/// Helper module for base64 encoding of byte arrays in JSON
mod base64_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use base64::Engine;
        serializer.serialize_str(&base64::engine::general_purpose::STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use base64::Engine;
        let s = String::deserialize(deserializer)?;
        base64::engine::general_purpose::STANDARD
            .decode(&s)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_message_serialization() {
        let msg = ClientMessage::ListDir {
            path: "/home".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("list_dir"));

        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            ClientMessage::ListDir { path } => assert_eq!(path, "/home"),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_server_message_serialization() {
        let msg = ServerMessage::Load(ServerLoad {
            cpu_percent: 75.5,
            ram_free_mb: 256,
            io_busy: false,
            hints: vec![LoadHint::ThrottleTransfers],
        });
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("load"));
        assert!(json.contains("throttle_transfers"));
    }
}
