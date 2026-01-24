//! pibox-server: WebSocket orchestration daemon for embedded devices
//!
//! This server sits between clients and the Filebrowser backend:
//! - Handles JWT authentication (Filebrowser has its own auth, we bypass it)
//! - Provides real-time updates via WebSocket
//! - Manages load and offloads heavy ops to capable clients
//! - Rate limits operations to protect embedded device CPU

mod handlers;
mod load;
mod state;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use pibox_core::{Config, JwtAuth};

use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "pibox_server=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = Config::load().unwrap_or_else(|e| {
        tracing::warn!("Failed to load config: {}, using defaults", e);
        Config::default()
    });

    // Initialize JWT auth
    let jwt_secret = if let Some(ref secret) = config.server.jwt_secret {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(secret)
            .expect("Invalid JWT secret (must be base64)")
    } else {
        // Generate random secret
        let secret = pibox_core::auth::generate_secret();
        tracing::info!("Generated random JWT secret (will change on restart)");
        secret.to_vec()
    };

    let jwt_auth = JwtAuth::new(
        &jwt_secret,
        Some(config.server.access_token_ttl),
        Some(config.server.refresh_token_ttl),
    );

    // Initialize Filebrowser client
    let mut fb_client = pibox_core::FilebrowserClient::new(&config.server.filebrowser_url);

    // For now, we'll use the server's Filebrowser token directly
    // In production, you'd want to configure this or use service auth
    tracing::info!("Filebrowser backend: {}", config.server.filebrowser_url);

    // Create shared application state
    let state = Arc::new(RwLock::new(AppState::new(
        jwt_auth,
        fb_client,
        config.server.max_concurrent_transfers,
        config.server.load_report_interval,
    )));

    // Start load monitor
    let load_state = Arc::clone(&state);
    tokio::spawn(async move {
        load::monitor_loop(load_state).await;
    });

    // Build router
    let app = Router::new()
        .route("/ws", get(handlers::ws_handler))
        .route("/health", get(handlers::health_handler))
        .route("/api/login", post(handlers::login_handler))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start server
    let addr = SocketAddr::from((
        config.server.listen_addr.parse::<std::net::IpAddr>()?,
        config.server.ws_port,
    ));
    tracing::info!("pibox-server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
