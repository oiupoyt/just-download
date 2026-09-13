mod config;
mod error;
mod routes;
mod services;
mod utils;

use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

use crate::config::AppConfig;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=info".into()),
        )
        .init();

    let config = AppConfig::from_env();

    // Ensure downloads directory exists
    if let Err(e) = tokio::fs::create_dir_all(&config.download_dir).await {
        warn!("Failed to create download directory: {}", e);
    }

    // Spawn periodic cleanup loop
    let cleanup_dir = config.download_dir.clone();
    let max_age = config.max_file_age_secs;
    let interval = config.cleanup_interval_secs;
    let max_mb = config.max_dir_size_mb;
    tokio::spawn(async move {
        run_cleanup_loop(cleanup_dir, max_age, interval, max_mb).await;
    });

    // Build Axum routers
    let info_routes = Router::new()
        .route("/detect", post(routes::info::detect))
        .route("/preview", post(routes::info::preview));

    let yt_routes = Router::new()
        .route("/video", post(routes::youtube::video))
        .route("/audio", post(routes::youtube::audio))
        .route("/thumbnail", post(routes::youtube::thumbnail))
        .route("/subtitles", post(routes::youtube::subtitles))
        .route("/channel-art", post(routes::youtube::channel_art));

    let ig_routes = Router::new()
        .route("/post", post(routes::instagram::post))
        .route("/reel", post(routes::instagram::reel))
        .route("/profile", post(routes::instagram::profile))
        .route("/thumbnail", post(routes::instagram::thumbnail));

    // Static file serving for /files/*
    let serve_dir = ServeDir::new(&config.download_dir);

    let cors = CorsLayer::new()
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::OPTIONS,
        ])
        .allow_origin(tower_http::cors::Any)
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ACCEPT,
            axum::http::header::AUTHORIZATION,
        ]);

    let app = Router::new()
        .route("/", get(root_handler))
        .route("/health", get(health_handler))
        .nest("/api/info", info_routes)
        .nest("/api/youtube", yt_routes)
        .nest("/api/instagram", ig_routes)
        .nest_service("/files", serve_dir)
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024))
        .layer(cors)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(config.clone());

    let addr: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .expect("Invalid host/port configuration");

    info!("🚀 just-download Rust backend listening on http://{}", addr);
    info!("⚡ yt-dlp binary resolved to: {}", config.ytdlp_bin);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {}: {}", addr, e));

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn root_handler() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "message": "just download stuff lol (rust engine)",
        "version": "2.0.0-rs"
    }))
}

async fn health_handler() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "engine": "rust",
        "uptime": "ok"
    }))
}

/// Periodic background task that removes files older than max_age_secs and enforces directory quota
async fn run_cleanup_loop(dir: PathBuf, max_age_secs: u64, interval_secs: u64, max_dir_size_mb: u64) {
    let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
    let max_bytes = max_dir_size_mb * 1024 * 1024;

    loop {
        ticker.tick().await;
        let mut files: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
        let mut total_size: u64 = 0;
        let now = std::time::SystemTime::now();

        if let Ok(mut entries) = tokio::fs::read_dir(&dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Ok(meta) = entry.metadata().await {
                    if meta.is_file() {
                        let path = entry.path();
                        let size = meta.len();
                        let modified = meta.modified().unwrap_or(now);
                        let elapsed = now.duration_since(modified).unwrap_or(Duration::ZERO);

                        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        let is_partial = file_name.ends_with(".part")
                            || file_name.ends_with(".ytdl")
                            || file_name.ends_with(".tmp")
                            || file_name.ends_with(".temp");

                        // If file is older than max_age or is a stale partial download older than 30s
                        if elapsed.as_secs() > max_age_secs || (is_partial && elapsed.as_secs() > 30) {
                            let _ = tokio::fs::remove_file(&path).await;
                            info!("Auto-cleaner removed expired file: {:?} ({} KB)", path, size / 1024);
                        } else {
                            total_size += size;
                            files.push((path, size, modified));
                        }
                    }
                }
            }
        }

        // If total downloads folder size exceeds quota, delete oldest files until under 50% quota
        if total_size > max_bytes {
            info!(
                "Downloads directory exceeded limit ({} MB / {} MB). Purging oldest files...",
                total_size / (1024 * 1024),
                max_dir_size_mb
            );
            files.sort_by_key(|f| f.2);
            let target_size = max_bytes / 2;
            for (path, size, _) in files {
                if total_size <= target_size {
                    break;
                }
                let _ = tokio::fs::remove_file(&path).await;
                info!("Purged file to free space: {:?}", path);
                total_size = total_size.saturating_sub(size);
            }
        }
    }
}

/// Graceful shutdown listener for SIGINT (Ctrl+C) and SIGTERM
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("Shutdown signal received, draining active requests...");
}
