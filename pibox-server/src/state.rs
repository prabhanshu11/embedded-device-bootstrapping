//! Shared server state

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

use pibox_core::{FilebrowserClient, JwtAuth, ServerMessage};

/// Connected client info
pub struct ConnectedClient {
    pub id: String,
    pub username: String,
    pub capabilities: Option<pibox_core::protocol::ClientCapabilities>,
    pub sender: broadcast::Sender<ServerMessage>,
}

/// Shared application state
pub struct AppState {
    /// JWT authentication handler
    pub jwt_auth: JwtAuth,

    /// Filebrowser backend client
    pub fb_client: FilebrowserClient,

    /// Connected WebSocket clients
    pub clients: HashMap<String, ConnectedClient>,

    /// Current server load
    pub load: pibox_core::protocol::ServerLoad,

    /// Operation rate limiter settings
    pub max_concurrent_transfers: u32,
    pub active_transfers: u32,

    /// Load report interval in seconds
    pub load_report_interval: u64,

    /// Broadcast channel for server-wide events
    pub event_tx: broadcast::Sender<ServerMessage>,
}

impl AppState {
    pub fn new(
        jwt_auth: JwtAuth,
        fb_client: FilebrowserClient,
        max_concurrent_transfers: u32,
        load_report_interval: u64,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(100);

        Self {
            jwt_auth,
            fb_client,
            clients: HashMap::new(),
            load: pibox_core::protocol::ServerLoad {
                cpu_percent: 0.0,
                ram_free_mb: 0,
                io_busy: false,
                hints: vec![],
            },
            max_concurrent_transfers,
            active_transfers: 0,
            load_report_interval,
            event_tx,
        }
    }

    /// Register a new client connection
    pub fn register_client(&mut self, id: String, username: String) -> broadcast::Receiver<ServerMessage> {
        let (sender, receiver) = broadcast::channel(32);

        self.clients.insert(
            id.clone(),
            ConnectedClient {
                id,
                username,
                capabilities: None,
                sender,
            },
        );

        receiver
    }

    /// Unregister a client
    pub fn unregister_client(&mut self, id: &str) {
        self.clients.remove(id);
    }

    /// Update client capabilities
    pub fn update_client_capabilities(&mut self, id: &str, caps: pibox_core::protocol::ClientCapabilities) {
        if let Some(client) = self.clients.get_mut(id) {
            client.capabilities = Some(caps);
        }
    }

    /// Check if we can start a new transfer
    pub fn can_start_transfer(&self) -> bool {
        self.active_transfers < self.max_concurrent_transfers
    }

    /// Increment active transfer count
    pub fn start_transfer(&mut self) -> bool {
        if self.can_start_transfer() {
            self.active_transfers += 1;
            true
        } else {
            false
        }
    }

    /// Decrement active transfer count
    pub fn end_transfer(&mut self) {
        if self.active_transfers > 0 {
            self.active_transfers -= 1;
        }
    }

    /// Broadcast message to all connected clients
    pub fn broadcast(&self, msg: ServerMessage) {
        let _ = self.event_tx.send(msg);
    }

    /// Find a capable client for offloading a task
    pub fn find_offload_candidate(&self, task: &pibox_core::protocol::OffloadTask) -> Option<&ConnectedClient> {
        self.clients.values().find(|client| {
            if let Some(ref caps) = client.capabilities {
                // Only offload to clients on AC power with spare resources
                if !caps.on_ac_power || caps.ram_free_mb < 500 {
                    return false;
                }

                match task {
                    pibox_core::protocol::OffloadTask::Thumbnail { .. } => {
                        caps.can_generate_thumbnails && (caps.has_gpu || caps.cpu_cores >= 4)
                    }
                    pibox_core::protocol::OffloadTask::Search { .. } => {
                        caps.can_search_locally && caps.cpu_cores >= 4
                    }
                }
            } else {
                false
            }
        })
    }
}
