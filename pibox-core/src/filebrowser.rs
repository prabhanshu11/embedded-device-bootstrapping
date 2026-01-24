//! Filebrowser REST API client
//!
//! Filebrowser is an open-source file manager that runs as a web service.
//! This client talks to its REST API for actual file operations.
//!
//! The pibox-server proxies through this client, adding:
//! - WebSocket real-time updates
//! - JWT authentication (separate from Filebrowser's auth)
//! - Rate limiting and load management

use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::state::{FileEntry, FileType};

#[derive(Debug, Error)]
pub enum FilebrowserError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Authentication failed")]
    AuthFailed,

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Server error: {0}")]
    ServerError(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

/// Filebrowser API client
pub struct FilebrowserClient {
    client: Client,
    base_url: String,
    token: Option<String>,
}

/// Filebrowser auth response
#[derive(Debug, Deserialize)]
struct AuthResponse {
    token: String,
}

/// Filebrowser resource response
#[derive(Debug, Deserialize)]
struct ResourceResponse {
    name: String,
    path: String,
    #[serde(rename = "isDir")]
    is_dir: bool,
    size: u64,
    #[serde(rename = "modified")]
    modified: String,
    #[serde(rename = "type")]
    mime_type: Option<String>,
    #[serde(default)]
    items: Vec<ResourceResponse>,
}

/// Filebrowser upload options
#[derive(Debug, Serialize)]
struct UploadOptions {
    override_existing: bool,
}

impl FilebrowserClient {
    /// Create new Filebrowser client
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            token: None,
        }
    }

    /// Authenticate with Filebrowser
    pub async fn login(&mut self, username: &str, password: &str) -> Result<(), FilebrowserError> {
        #[derive(Serialize)]
        struct LoginRequest<'a> {
            username: &'a str,
            password: &'a str,
        }

        let resp = self
            .client
            .post(format!("{}/api/login", self.base_url))
            .json(&LoginRequest { username, password })
            .send()
            .await?;

        if resp.status() == reqwest::StatusCode::FORBIDDEN {
            return Err(FilebrowserError::AuthFailed);
        }

        let auth: AuthResponse = resp.json().await?;
        self.token = Some(auth.token);
        Ok(())
    }

    /// Set auth token directly (if already have one)
    pub fn set_token(&mut self, token: String) {
        self.token = Some(token);
    }

    /// List directory contents
    pub async fn list_dir(&self, path: &str) -> Result<Vec<FileEntry>, FilebrowserError> {
        let path = if path.is_empty() || path == "/" { "" } else { path };
        let url = format!("{}/api/resources{}", self.base_url, path);

        let resp = self.authed_request(reqwest::Method::GET, &url).send().await?;

        self.handle_error_status(&resp, path).await?;

        let resource: ResourceResponse = resp.json().await?;

        Ok(resource
            .items
            .into_iter()
            .map(|r| self.resource_to_entry(r))
            .collect())
    }

    /// Get file/directory info
    pub async fn get_info(&self, path: &str) -> Result<FileEntry, FilebrowserError> {
        let url = format!("{}/api/resources{}", self.base_url, path);

        let resp = self.authed_request(reqwest::Method::GET, &url).send().await?;

        self.handle_error_status(&resp, path).await?;

        let resource: ResourceResponse = resp.json().await?;
        Ok(self.resource_to_entry(resource))
    }

    /// Download file contents
    pub async fn download(&self, path: &str) -> Result<Vec<u8>, FilebrowserError> {
        let url = format!("{}/api/raw{}", self.base_url, path);

        let resp = self.authed_request(reqwest::Method::GET, &url).send().await?;

        self.handle_error_status(&resp, path).await?;

        Ok(resp.bytes().await?.to_vec())
    }

    /// Upload file
    pub async fn upload(&self, path: &str, content: &[u8], override_existing: bool) -> Result<(), FilebrowserError> {
        let url = format!(
            "{}/api/resources{}?override={}",
            self.base_url, path, override_existing
        );

        let resp = self
            .authed_request(reqwest::Method::POST, &url)
            .body(content.to_vec())
            .send()
            .await?;

        self.handle_error_status(&resp, path).await?;
        Ok(())
    }

    /// Delete file or directory
    pub async fn delete(&self, path: &str) -> Result<(), FilebrowserError> {
        let url = format!("{}/api/resources{}", self.base_url, path);

        let resp = self.authed_request(reqwest::Method::DELETE, &url).send().await?;

        self.handle_error_status(&resp, path).await?;
        Ok(())
    }

    /// Rename/move file or directory
    pub async fn rename(&self, from: &str, to: &str) -> Result<(), FilebrowserError> {
        let url = format!("{}/api/resources{}", self.base_url, from);

        #[derive(Serialize)]
        struct RenameRequest<'a> {
            action: &'a str,
            destination: &'a str,
        }

        let resp = self
            .authed_request(reqwest::Method::PATCH, &url)
            .json(&RenameRequest {
                action: "rename",
                destination: to,
            })
            .send()
            .await?;

        self.handle_error_status(&resp, from).await?;
        Ok(())
    }

    /// Create directory
    pub async fn mkdir(&self, path: &str) -> Result<(), FilebrowserError> {
        let url = format!("{}/api/resources{}/?override=false", self.base_url, path);

        let resp = self
            .authed_request(reqwest::Method::POST, &url)
            .header("Content-Length", "0")
            .send()
            .await?;

        self.handle_error_status(&resp, path).await?;
        Ok(())
    }

    // Private helpers

    fn authed_request(&self, method: reqwest::Method, url: &str) -> reqwest::RequestBuilder {
        let mut req = self.client.request(method, url);

        if let Some(ref token) = self.token {
            req = req.header("X-Auth", token);
        }

        req
    }

    async fn handle_error_status(&self, resp: &reqwest::Response, path: &str) -> Result<(), FilebrowserError> {
        match resp.status() {
            s if s.is_success() => Ok(()),
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
                Err(FilebrowserError::PermissionDenied(path.to_string()))
            }
            reqwest::StatusCode::NOT_FOUND => Err(FilebrowserError::NotFound(path.to_string())),
            s => Err(FilebrowserError::ServerError(format!(
                "HTTP {}: {}",
                s.as_u16(),
                path
            ))),
        }
    }

    fn resource_to_entry(&self, r: ResourceResponse) -> FileEntry {
        let modified = chrono::DateTime::parse_from_rfc3339(&r.modified)
            .map(|dt| dt.timestamp())
            .unwrap_or(0);

        FileEntry {
            name: r.name,
            path: r.path,
            file_type: if r.is_dir {
                FileType::Directory
            } else {
                FileType::File
            },
            size: r.size,
            modified,
            mime_type: r.mime_type,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = FilebrowserClient::new("http://localhost:8080");
        assert_eq!(client.base_url, "http://localhost:8080");
    }

    #[test]
    fn test_url_normalization() {
        let client = FilebrowserClient::new("http://localhost:8080/");
        assert_eq!(client.base_url, "http://localhost:8080");
    }
}
