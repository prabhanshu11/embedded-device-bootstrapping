//! HTTP and WebSocket handlers

use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use pibox_core::{ClientMessage, ServerMessage, TokenPair};

use crate::state::AppState;

pub type SharedState = Arc<RwLock<AppState>>;

/// Health check endpoint
pub async fn health_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "pibox-server"
    }))
}

/// Login request body
#[derive(Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

/// Login response
#[derive(Serialize)]
pub struct LoginResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

/// HTTP login endpoint (alternative to WebSocket login)
pub async fn login_handler(
    State(state): State<SharedState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    // TODO: In production, validate against actual user database
    // For now, accept any credentials for testing
    if req.username.is_empty() || req.password.is_empty() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let s = state.read().await;
    let tokens = s
        .jwt_auth
        .generate_tokens(&req.username, None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(LoginResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        expires_in: tokens.expires_in,
    }))
}

/// WebSocket upgrade handler
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<SharedState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

/// Handle WebSocket connection
async fn handle_websocket(socket: WebSocket, state: SharedState) {
    let (mut sender, mut receiver) = socket.split();

    // Generate client ID
    let client_id = uuid::Uuid::new_v4().to_string();
    tracing::info!("New WebSocket connection: {}", client_id);

    // Wait for authentication
    let username = match wait_for_auth(&mut receiver, &state).await {
        Some(u) => u,
        None => {
            tracing::warn!("Client {} failed authentication", client_id);
            let _ = sender
                .send(Message::Text(
                    serde_json::to_string(&ServerMessage::AuthError {
                        message: "Authentication required".to_string(),
                    })
                    .unwrap().into(),
                ))
                .await;
            return;
        }
    };

    tracing::info!("Client {} authenticated as {}", client_id, username);

    // Register client and get event receiver
    let mut event_rx = {
        let mut s = state.write().await;
        s.register_client(client_id.clone(), username.clone())
    };

    // Subscribe to broadcast events
    let broadcast_rx = {
        let s = state.read().await;
        s.event_tx.subscribe()
    };

    // Spawn task to forward broadcast events to client
    let sender_clone = Arc::new(tokio::sync::Mutex::new(sender));
    let sender_for_broadcast = Arc::clone(&sender_clone);
    let broadcast_handle = tokio::spawn(async move {
        let mut rx = broadcast_rx;
        while let Ok(msg) = rx.recv().await {
            let mut s = sender_for_broadcast.lock().await;
            if let Ok(json) = serde_json::to_string(&msg) {
                if s.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Main message loop
    loop {
        tokio::select! {
            // Handle incoming messages
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                            let response = handle_client_message(&client_id, client_msg, &state).await;
                            if let Some(resp) = response {
                                let mut s = sender_clone.lock().await;
                                if let Ok(json) = serde_json::to_string(&resp) {
                                    if s.send(Message::Text(json.into())).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        tracing::info!("Client {} disconnected", client_id);
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let mut s = sender_clone.lock().await;
                        let _ = s.send(Message::Pong(data)).await;
                    }
                    _ => {}
                }
            }
        }
    }

    // Cleanup
    broadcast_handle.abort();
    {
        let mut s = state.write().await;
        s.unregister_client(&client_id);
    }
    tracing::info!("Client {} cleaned up", client_id);
}

/// Wait for client to authenticate
async fn wait_for_auth(
    receiver: &mut futures::stream::SplitStream<WebSocket>,
    state: &SharedState,
) -> Option<String> {
    // Give client 30 seconds to authenticate
    let timeout = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                if let Ok(ClientMessage::Login { username, password }) = serde_json::from_str(&text) {
                    // TODO: Validate against actual user store
                    if !username.is_empty() && !password.is_empty() {
                        return Some(username);
                    }
                }
            }
        }
        None
    });

    timeout.await.ok().flatten()
}

/// Handle a client message and return optional response
async fn handle_client_message(
    client_id: &str,
    msg: ClientMessage,
    state: &SharedState,
) -> Option<ServerMessage> {
    match msg {
        ClientMessage::Ping => Some(ServerMessage::Pong),

        ClientMessage::RefreshToken { refresh_token } => {
            let s = state.read().await;
            match s.jwt_auth.refresh_tokens(&refresh_token) {
                Ok(tokens) => Some(ServerMessage::AuthSuccess(pibox_core::protocol::TokenPairResponse {
                    access_token: tokens.access_token,
                    refresh_token: tokens.refresh_token,
                    expires_in: tokens.expires_in,
                })),
                Err(e) => Some(ServerMessage::AuthError {
                    message: e.to_string(),
                }),
            }
        }

        ClientMessage::ListDir { path } => {
            let s = state.read().await;
            match s.fb_client.list_dir(&path).await {
                Ok(entries) => Some(ServerMessage::DirListing {
                    path,
                    entries: entries
                        .into_iter()
                        .map(|e| {
                            let is_dir = e.is_dir();
                            pibox_core::protocol::FileEntryResponse {
                                name: e.name,
                                path: e.path,
                                is_dir,
                                size: e.size,
                                modified: e.modified,
                                mime_type: e.mime_type,
                            }
                        })
                        .collect(),
                }),
                Err(e) => Some(ServerMessage::OpError {
                    op: "list".to_string(),
                    path,
                    message: e.to_string(),
                }),
            }
        }

        ClientMessage::Download { path } => {
            // Check rate limit
            {
                let mut s = state.write().await;
                if !s.start_transfer() {
                    return Some(ServerMessage::OpError {
                        op: "download".to_string(),
                        path,
                        message: "Too many concurrent transfers".to_string(),
                    });
                }
            }

            let result = {
                let s = state.read().await;
                s.fb_client.download(&path).await
            };

            // End transfer
            {
                let mut s = state.write().await;
                s.end_transfer();
            }

            match result {
                Ok(content) => Some(ServerMessage::FileContent {
                    path,
                    content,
                    mime_type: None, // TODO: detect mime type
                }),
                Err(e) => Some(ServerMessage::OpError {
                    op: "download".to_string(),
                    path,
                    message: e.to_string(),
                }),
            }
        }

        ClientMessage::Upload { path, content } => {
            // Check rate limit
            {
                let mut s = state.write().await;
                if !s.start_transfer() {
                    return Some(ServerMessage::OpError {
                        op: "upload".to_string(),
                        path,
                        message: "Too many concurrent transfers".to_string(),
                    });
                }
            }

            let result = {
                let s = state.read().await;
                s.fb_client.upload(&path, &content, true).await
            };

            // End transfer
            {
                let mut s = state.write().await;
                s.end_transfer();
            }

            match result {
                Ok(()) => {
                    // Broadcast file created event
                    let s = state.read().await;
                    s.broadcast(ServerMessage::FsEvent(pibox_core::protocol::FsEvent::Created {
                        path: path.clone(),
                        is_dir: false,
                    }));
                    Some(ServerMessage::OpSuccess {
                        op: "upload".to_string(),
                        path,
                    })
                }
                Err(e) => Some(ServerMessage::OpError {
                    op: "upload".to_string(),
                    path,
                    message: e.to_string(),
                }),
            }
        }

        ClientMessage::Delete { path } => {
            let result = {
                let s = state.read().await;
                s.fb_client.delete(&path).await
            };

            match result {
                Ok(()) => {
                    let s = state.read().await;
                    s.broadcast(ServerMessage::FsEvent(pibox_core::protocol::FsEvent::Deleted {
                        path: path.clone(),
                    }));
                    Some(ServerMessage::OpSuccess {
                        op: "delete".to_string(),
                        path,
                    })
                }
                Err(e) => Some(ServerMessage::OpError {
                    op: "delete".to_string(),
                    path,
                    message: e.to_string(),
                }),
            }
        }

        ClientMessage::Rename { from, to } => {
            let result = {
                let s = state.read().await;
                s.fb_client.rename(&from, &to).await
            };

            match result {
                Ok(()) => {
                    let s = state.read().await;
                    s.broadcast(ServerMessage::FsEvent(pibox_core::protocol::FsEvent::Renamed {
                        from: from.clone(),
                        to: to.clone(),
                    }));
                    Some(ServerMessage::OpSuccess {
                        op: "rename".to_string(),
                        path: from,
                    })
                }
                Err(e) => Some(ServerMessage::OpError {
                    op: "rename".to_string(),
                    path: from,
                    message: e.to_string(),
                }),
            }
        }

        ClientMessage::Mkdir { path } => {
            let result = {
                let s = state.read().await;
                s.fb_client.mkdir(&path).await
            };

            match result {
                Ok(()) => {
                    let s = state.read().await;
                    s.broadcast(ServerMessage::FsEvent(pibox_core::protocol::FsEvent::Created {
                        path: path.clone(),
                        is_dir: true,
                    }));
                    Some(ServerMessage::OpSuccess {
                        op: "mkdir".to_string(),
                        path,
                    })
                }
                Err(e) => Some(ServerMessage::OpError {
                    op: "mkdir".to_string(),
                    path,
                    message: e.to_string(),
                }),
            }
        }

        ClientMessage::Capabilities(caps) => {
            let mut s = state.write().await;
            s.update_client_capabilities(client_id, caps);
            None // No response needed
        }

        ClientMessage::OffloadResult { task_id, result } => {
            // Handle offload result from client
            tracing::info!("Received offload result for task {}", task_id);
            // TODO: Route result to original requester
            None
        }

        // Already handled in wait_for_auth
        ClientMessage::Login { .. } => None,
    }
}
