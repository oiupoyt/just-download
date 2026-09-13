use crate::error::AppError;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{error, info};

pub struct YtDlp;

impl YtDlp {
    /// Detects if node runtime exists for yt-dlp javascript engine
    fn has_node() -> bool {
        which::which("node").is_ok()
    }

    /// Spawns yt-dlp and extracts JSON metadata
    pub async fn dump_json(bin: &str, url: &str) -> Result<serde_json::Value, AppError> {
        let mut cmd = Command::new(bin);
        cmd.arg("--dump-json")
            .arg("--skip-download")
            .arg("--no-warnings")
            .arg("--no-playlist");

        if Self::has_node() {
            cmd.arg("--js-runtimes").arg("node");
        }

        cmd.arg(url);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let output = cmd.output().await.map_err(|e| {
            AppError::Internal(format!("Failed to execute yt-dlp: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("yt-dlp error: {}", stderr);
            return Err(AppError::Internal(format!(
                "yt-dlp failed: {}",
                stderr.lines().next().unwrap_or("Unknown error")
            )));
        }

        let json_val: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| AppError::Internal(format!("Failed to parse yt-dlp metadata: {}", e)))?;

        Ok(json_val)
    }

    /// Runs yt-dlp with custom arguments
    pub async fn execute(bin: &str, args: &[&str]) -> Result<(), AppError> {
        let mut cmd = Command::new(bin);
        cmd.args(args);
        cmd.arg("--no-warnings").arg("--no-playlist");

        if Self::has_node() {
            cmd.arg("--js-runtimes").arg("node");
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        info!("Executing yt-dlp with {} args", args.len());
        let output = cmd.output().await.map_err(|e| {
            AppError::Internal(format!("Failed to run yt-dlp: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("yt-dlp execution failed: {}", stderr);
            return Err(AppError::Internal(format!(
                "Download failed: {}",
                stderr.lines().last().unwrap_or("yt-dlp error")
            )));
        }

        Ok(())
    }

    /// Trims video/audio using ffmpeg stream copy
    pub async fn trim_media(
        src: &Path,
        dest: &Path,
        start_time: Option<&str>,
        end_time: Option<&str>,
    ) -> Result<(), AppError> {
        let mut cmd = Command::new("ffmpeg");
        cmd.arg("-y");

        if let Some(start) = start_time {
            cmd.arg("-ss").arg(start);
        }
        cmd.arg("-i").arg(src);

        if let Some(end) = end_time {
            cmd.arg("-to").arg(end);
        }
        cmd.arg("-c").arg("copy").arg(dest);

        cmd.stdout(Stdio::null()).stderr(Stdio::piped());

        let output = cmd.output().await.map_err(|e| {
            AppError::Internal(format!("Failed to execute ffmpeg: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::Internal(format!("ffmpeg trim failed: {}", stderr)));
        }

        Ok(())
    }

    /// Finds the first file in a directory that matches a prefix
    pub fn find_file_with_prefix(dir: &Path, prefix: &str) -> Option<PathBuf> {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with(prefix) && entry.path().is_file() {
                    return Some(entry.path());
                }
            }
        }
        None
    }
}
