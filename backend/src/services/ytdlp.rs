use crate::error::AppError;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{error, info};

pub struct YtDlp;

impl YtDlp {
    /// Detects and configures node runtime for yt-dlp javascript engine
    fn setup_js_runtime(cmd: &mut Command) {
        if which::which("node").is_ok() {
            cmd.arg("--js-runtimes").arg("node");
        } else if std::path::Path::new("/data/data/com.termux/files/usr/bin/node").exists() {
            cmd.arg("--js-runtimes")
                .arg("node:/data/data/com.termux/files/usr/bin/node");
        }
    }

    /// Spawns yt-dlp and extracts JSON metadata
    pub async fn dump_json(bin: &str, url: &str) -> Result<serde_json::Value, AppError> {
        let mut cmd = Command::new(bin);
        cmd.arg("--dump-json")
            .arg("--skip-download")
            .arg("--no-warnings")
            .arg("--no-playlist")
            .arg("--extractor-args")
            .arg("youtube:player_client=tv,android,mweb");

        Self::setup_js_runtime(&mut cmd);

        // Use "--" delimiter to guarantee the URL is never parsed as a CLI flag
        cmd.arg("--");
        cmd.arg(url);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let output = cmd.output().await.map_err(|e| {
            AppError::Internal(format!("Failed to execute yt-dlp: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("yt-dlp error: {}", stderr);
            let first_line = stderr
                .lines()
                .find(|l| l.contains("ERROR:"))
                .unwrap_or_else(|| stderr.lines().next().unwrap_or("Unknown error"));

            let friendly_msg = if stderr.contains("Instagram") && stderr.contains("empty media") {
                "Instagram restricted this media. Ensure the post is public or try again later.".to_string()
            } else if stderr.contains("not a bot") {
                "YouTube verification challenge encountered. Please retry shortly.".to_string()
            } else {
                format!("yt-dlp error: {}", first_line)
            };

            return Err(AppError::Internal(friendly_msg));
        }

        let json_val: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| AppError::Internal(format!("Failed to parse yt-dlp metadata: {}", e)))?;

        Ok(json_val)
    }

    /// Runs yt-dlp with custom arguments and a target URL
    pub async fn execute(bin: &str, args: &[&str], url: &str) -> Result<(), AppError> {
        let mut cmd = Command::new(bin);
        cmd.args(args);
        cmd.arg("--no-warnings")
            .arg("--no-playlist")
            .arg("--extractor-args")
            .arg("youtube:player_client=tv,android,mweb");

        Self::setup_js_runtime(&mut cmd);

        // Use "--" delimiter to guarantee the URL is never parsed as a CLI flag
        cmd.arg("--");
        cmd.arg(url);

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        info!("Executing yt-dlp with {} flags against target", args.len());
        let output = cmd.output().await.map_err(|e| {
            AppError::Internal(format!("Failed to run yt-dlp: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("yt-dlp execution failed: {}", stderr);
            let first_line = stderr
                .lines()
                .find(|l| l.contains("ERROR:"))
                .unwrap_or_else(|| stderr.lines().last().unwrap_or("Download failed"));

            let friendly_msg = if stderr.contains("Instagram") && stderr.contains("empty media") {
                "Instagram restricted this media. Ensure the post is public or try again later.".to_string()
            } else if stderr.contains("not a bot") {
                "YouTube verification challenge encountered. Please retry shortly.".to_string()
            } else {
                format!("Download error: {}", first_line)
            };

            return Err(AppError::Internal(friendly_msg));
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
            if crate::utils::is_valid_time_format(start) {
                cmd.arg("-ss").arg(start);
            }
        }
        cmd.arg("-i").arg(src);

        if let Some(end) = end_time {
            if crate::utils::is_valid_time_format(end) {
                cmd.arg("-to").arg(end);
            }
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
