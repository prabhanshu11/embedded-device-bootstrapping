//! Configuration management for pibox
//!
//! Config files are stored in platform-appropriate locations:
//! - Linux: ~/.config/pibox/
//! - macOS: ~/Library/Application Support/pibox/
//! - Windows: %APPDATA%\pibox\

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse config: {0}")]
    ParseError(#[from] toml::de::Error),

    #[error("Failed to serialize config: {0}")]
    SerializeError(#[from] toml::ser::Error),

    #[error("Config directory not found")]
    NoDirFound,
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Server configuration (for pibox-server)
    #[serde(default)]
    pub server: ServerConfig,

    /// Client configuration (for pibox-tui and pibox-gui)
    #[serde(default)]
    pub client: ClientConfig,

    /// Known devices
    #[serde(default)]
    pub devices: Vec<DeviceConfig>,
}

/// Server-side configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// WebSocket listen address
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,

    /// WebSocket port
    #[serde(default = "default_ws_port")]
    pub ws_port: u16,

    /// Filebrowser backend URL
    #[serde(default = "default_filebrowser_url")]
    pub filebrowser_url: String,

    /// JWT secret (base64 encoded)
    /// If not set, a random secret is generated on first run
    pub jwt_secret: Option<String>,

    /// Access token TTL in seconds
    #[serde(default = "default_access_ttl")]
    pub access_token_ttl: u64,

    /// Refresh token TTL in seconds
    #[serde(default = "default_refresh_ttl")]
    pub refresh_token_ttl: u64,

    /// Maximum concurrent file transfers
    #[serde(default = "default_max_transfers")]
    pub max_concurrent_transfers: u32,

    /// Load reporting interval in seconds
    #[serde(default = "default_load_interval")]
    pub load_report_interval: u64,
}

/// Client-side configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    /// Default server to connect to
    pub default_device: Option<String>,

    /// Theme preference
    #[serde(default)]
    pub theme: Theme,

    /// Show hidden files
    #[serde(default)]
    pub show_hidden: bool,

    /// Confirm before delete
    #[serde(default = "default_true")]
    pub confirm_delete: bool,

    /// Enable vim-style keybindings
    #[serde(default = "default_true")]
    pub vim_mode: bool,

    /// TUI-specific settings
    #[serde(default)]
    pub tui: TuiConfig,

    /// GUI-specific settings
    #[serde(default)]
    pub gui: GuiConfig,
}

/// Known device configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    /// Device display name
    pub name: String,

    /// WebSocket URL
    pub url: String,

    /// Username (stored separately in keyring for security)
    pub username: Option<String>,

    /// Device type for display
    #[serde(default)]
    pub device_type: DeviceType,
}

/// Device type classification
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    #[default]
    Generic,
    Nas,
    Camera,
    Sensor,
}

/// Theme preference
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// TUI-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiConfig {
    /// Use true color (24-bit)
    #[serde(default = "default_true")]
    pub true_color: bool,

    /// Enable mouse support
    #[serde(default = "default_true")]
    pub mouse: bool,

    /// Enable image preview (sixel/kitty)
    #[serde(default)]
    pub image_preview: bool,
}

/// GUI-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiConfig {
    /// Window width
    #[serde(default = "default_window_width")]
    pub window_width: u32,

    /// Window height
    #[serde(default = "default_window_height")]
    pub window_height: u32,

    /// Enable thumbnail preview
    #[serde(default = "default_true")]
    pub thumbnails: bool,

    /// Thumbnail cache size in MB
    #[serde(default = "default_thumb_cache")]
    pub thumbnail_cache_mb: u32,
}

// Default value functions
fn default_listen_addr() -> String {
    "0.0.0.0".to_string()
}
fn default_ws_port() -> u16 {
    crate::DEFAULT_WS_PORT
}
fn default_filebrowser_url() -> String {
    format!("http://127.0.0.1:{}", crate::DEFAULT_FILEBROWSER_PORT)
}
fn default_access_ttl() -> u64 {
    900 // 15 minutes
}
fn default_refresh_ttl() -> u64 {
    604800 // 7 days
}
fn default_max_transfers() -> u32 {
    3
}
fn default_load_interval() -> u64 {
    5
}
fn default_true() -> bool {
    true
}
fn default_window_width() -> u32 {
    1200
}
fn default_window_height() -> u32 {
    800
}
fn default_thumb_cache() -> u32 {
    100
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: default_listen_addr(),
            ws_port: default_ws_port(),
            filebrowser_url: default_filebrowser_url(),
            jwt_secret: None,
            access_token_ttl: default_access_ttl(),
            refresh_token_ttl: default_refresh_ttl(),
            max_concurrent_transfers: default_max_transfers(),
            load_report_interval: default_load_interval(),
        }
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            default_device: None,
            theme: Theme::default(),
            show_hidden: false,
            confirm_delete: true,
            vim_mode: true,
            tui: TuiConfig::default(),
            gui: GuiConfig::default(),
        }
    }
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            true_color: true,
            mouse: true,
            image_preview: false,
        }
    }
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            window_width: default_window_width(),
            window_height: default_window_height(),
            thumbnails: true,
            thumbnail_cache_mb: default_thumb_cache(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            client: ClientConfig::default(),
            devices: Vec::new(),
        }
    }
}

impl Config {
    /// Get config directory path
    pub fn config_dir() -> Result<PathBuf, ConfigError> {
        dirs::config_dir()
            .map(|p| p.join("pibox"))
            .ok_or(ConfigError::NoDirFound)
    }

    /// Get config file path
    pub fn config_path() -> Result<PathBuf, ConfigError> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    /// Load config from default location
    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::config_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Load config from specific path
    pub fn load_from(path: &std::path::Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save config to default location
    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::config_path()?;

        // Create directory if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Save config to specific path
    pub fn save_to(&self, path: &std::path::Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get device config by name
    pub fn get_device(&self, name: &str) -> Option<&DeviceConfig> {
        self.devices.iter().find(|d| d.name == name)
    }

    /// Add or update device
    pub fn upsert_device(&mut self, device: DeviceConfig) {
        if let Some(existing) = self.devices.iter_mut().find(|d| d.name == device.name) {
            *existing = device;
        } else {
            self.devices.push(device);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.server.ws_port, crate::DEFAULT_WS_PORT);
        assert!(config.client.vim_mode);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml = toml::to_string_pretty(&config).unwrap();
        assert!(toml.contains("[server]"));

        let parsed: Config = toml::from_str(&toml).unwrap();
        assert_eq!(parsed.server.ws_port, config.server.ws_port);
    }

    #[test]
    fn test_device_upsert() {
        let mut config = Config::default();

        config.upsert_device(DeviceConfig {
            name: "nas".to_string(),
            url: "ws://192.0.2.10:9280".to_string(),
            username: Some("admin".to_string()),
            device_type: DeviceType::Nas,
        });

        assert_eq!(config.devices.len(), 1);

        // Update existing
        config.upsert_device(DeviceConfig {
            name: "nas".to_string(),
            url: "ws://192.0.2.11:9280".to_string(),
            username: Some("admin".to_string()),
            device_type: DeviceType::Nas,
        });

        assert_eq!(config.devices.len(), 1);
        assert!(config.devices[0].url.contains("192.0.2.11"));
    }
}
