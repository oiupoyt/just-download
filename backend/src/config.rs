use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub download_dir: PathBuf,
    pub max_file_age_secs: u64,
    pub cleanup_interval_secs: u64,
    pub max_dir_size_mb: u64,
    pub semaphore: Arc<Semaphore>,
    pub ytdlp_bin: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8000);
        let download_dir = PathBuf::from(
            std::env::var("DOWNLOAD_DIR").unwrap_or_else(|_| "downloads".to_string()),
        );
        // Automatically delete files older than 60 seconds (stream completes within seconds)
        let max_file_age_secs = std::env::var("MAX_FILE_AGE_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);
        // Run background cleaner every 30 seconds
        let cleanup_interval_secs = std::env::var("CLEANUP_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);
        // Max total directory size: 200 MB quota to prevent disk bloat
        let max_dir_size_mb = std::env::var("MAX_DIR_SIZE_MB")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(200);

        // Limit concurrent downloads on low-memory servers (default 2)
        let max_concurrent = std::env::var("MAX_CONCURRENT_DOWNLOADS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);

        // Find yt-dlp binary
        let ytdlp_bin = if let Ok(custom) = std::env::var("YTDLP_PATH") {
            custom
        } else if which::which("yt-dlp").is_ok() {
            "yt-dlp".to_string()
        } else if std::path::Path::new("./yt-dlp").exists() {
            "./yt-dlp".to_string()
        } else {
            "yt-dlp".to_string()
        };

        Self {
            host,
            port,
            download_dir,
            max_file_age_secs,
            cleanup_interval_secs,
            max_dir_size_mb,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            ytdlp_bin,
        }
    }
}
