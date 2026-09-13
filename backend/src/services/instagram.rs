use crate::error::AppError;
use crate::services::ytdlp::YtDlp;
use crate::utils::{format_duration, safe_filename};
use serde::Serialize;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Serialize)]
pub struct IgPostInfoResponse {
    #[serde(rename = "type")]
    pub media_type: String,
    pub title: String,
    pub thumbnail: Option<String>,
    pub duration: Option<f64>,
    pub duration_str: Option<String>,
    pub owner: Option<String>,
    pub likes: Option<u64>,
}

#[derive(Serialize)]
pub struct IgProfileInfoResponse {
    #[serde(rename = "type")]
    pub media_type: String,
    pub title: String,
    pub thumbnail: Option<String>,
    pub owner: String,
    pub likes: Option<u64>,
}

pub struct InstagramService;

impl InstagramService {
    pub async fn fetch_post_info(bin: &str, url: &str) -> Result<IgPostInfoResponse, AppError> {
        let json = YtDlp::dump_json(bin, url).await?;

        let title = json
            .get("title")
            .or_else(|| json.get("uploader"))
            .and_then(|v| v.as_str())
            .unwrap_or("Instagram post")
            .to_string();

        let thumbnail = json.get("thumbnail").and_then(|v| v.as_str()).map(String::from);
        let duration = json.get("duration").and_then(|v| v.as_f64());
        let duration_str = duration.map(|d| format_duration(Some(d)));
        let owner = json.get("uploader").and_then(|v| v.as_str()).map(String::from);
        let likes = json.get("like_count").and_then(|v| v.as_u64());

        Ok(IgPostInfoResponse {
            media_type: "video".to_string(),
            title,
            thumbnail,
            duration,
            duration_str,
            owner,
            likes,
        })
    }

    pub async fn fetch_profile_info(bin: &str, username: &str) -> Result<IgProfileInfoResponse, AppError> {
        let profile_url = format!("https://www.instagram.com/{}/", username);
        let json = YtDlp::dump_json(bin, &profile_url).await?;

        let thumbnail = json.get("thumbnail").and_then(|v| v.as_str()).map(String::from);
        let followers = json.get("channel_follower_count").and_then(|v| v.as_u64());

        Ok(IgProfileInfoResponse {
            media_type: "profile".to_string(),
            title: format!("@{}", username),
            thumbnail,
            owner: username.to_string(),
            likes: followers,
        })
    }

    pub async fn download_post(
        bin: &str,
        download_dir: &Path,
        url: &str,
        fmt: &str,
        start_time: Option<&str>,
        end_time: Option<&str>,
    ) -> Result<(PathBuf, String), AppError> {
        let uid = &Uuid::new_v4().to_string()[..10];
        let out_prefix = format!("ig_post_{}", uid);
        let out_tmpl = format!("{}/{}.%(ext)s", download_dir.display(), out_prefix);

        let args = [
            "-f", "bestvideo+bestaudio/best",
            "--merge-output-format", fmt,
            "-o", &out_tmpl,
        ];

        YtDlp::execute(bin, &args, url).await?;

        let mut final_file = YtDlp::find_file_with_prefix(download_dir, &out_prefix)
            .ok_or_else(|| AppError::NotFound("Instagram post file not found".to_string()))?;

        if start_time.is_some() || end_time.is_some() {
            let trimmed = download_dir.join(format!("ig_post_{}_trim.mp4", uid));
            if YtDlp::trim_media(&final_file, &trimmed, start_time, end_time).await.is_ok() {
                let _ = tokio::fs::remove_file(&final_file).await;
                final_file = trimmed;
            }
        }

        let download_name = format!("ig_post_{}.{}", uid, fmt);
        Ok((final_file, download_name))
    }

    pub async fn download_reel(
        bin: &str,
        download_dir: &Path,
        url: &str,
        start_time: Option<&str>,
        end_time: Option<&str>,
    ) -> Result<(PathBuf, String), AppError> {
        let uid = &Uuid::new_v4().to_string()[..10];
        let out_prefix = format!("ig_reel_{}", uid);
        let out_tmpl = format!("{}/{}.%(ext)s", download_dir.display(), out_prefix);

        let args = [
            "-f", "bestvideo+bestaudio/best",
            "--merge-output-format", "mp4",
            "-o", &out_tmpl,
        ];

        YtDlp::execute(bin, &args, url).await?;

        let mut final_file = YtDlp::find_file_with_prefix(download_dir, &out_prefix)
            .ok_or_else(|| AppError::NotFound("Instagram reel file not found".to_string()))?;

        if start_time.is_some() || end_time.is_some() {
            let trimmed = download_dir.join(format!("ig_reel_{}_trim.mp4", uid));
            if YtDlp::trim_media(&final_file, &trimmed, start_time, end_time).await.is_ok() {
                let _ = tokio::fs::remove_file(&final_file).await;
                final_file = trimmed;
            }
        }

        let download_name = format!("ig_reel_{}.mp4", uid);
        Ok((final_file, download_name))
    }

    pub async fn download_thumbnail(
        bin: &str,
        download_dir: &Path,
        url: &str,
    ) -> Result<(PathBuf, String), AppError> {
        let uid = &Uuid::new_v4().to_string()[..10];
        let json = YtDlp::dump_json(bin, url).await?;

        let thumb_url = json
            .get("thumbnail")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::NotFound("No thumbnail found for Instagram post".to_string()))?;

        let parsed_url = crate::utils::parse_safe_url(thumb_url)
            .map_err(|e| AppError::BadRequest(format!("Invalid thumbnail URL: {}", e)))?;

        let filename = format!("ig_thumb_{}.jpg", uid);
        let dest = download_dir.join(&filename);

        let client = reqwest::Client::new();
        let bytes = client
            .get(parsed_url)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to fetch thumbnail: {}", e)))?
            .bytes()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to read thumbnail bytes: {}", e)))?;

        tokio::fs::write(&dest, bytes).await?;

        Ok((dest, filename))
    }

    pub async fn download_profile(
        bin: &str,
        download_dir: &Path,
        username: &str,
    ) -> Result<(PathBuf, String), AppError> {
        let uid = &Uuid::new_v4().to_string()[..10];
        let profile_url = format!("https://www.instagram.com/{}/", username);
        let json = YtDlp::dump_json(bin, &profile_url).await?;

        let thumb_url = json
            .get("thumbnail")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::NotFound(format!("Could not find profile picture for @{}", username)))?;

        let parsed_url = crate::utils::parse_safe_url(thumb_url)
            .map_err(|e| AppError::BadRequest(format!("Invalid profile picture URL: {}", e)))?;

        let safe_user = safe_filename(username);
        let filename = format!("{}_profile.jpg", safe_user);
        let dest = download_dir.join(format!("ig_pfp_{}_{}.jpg", safe_user, uid));

        let client = reqwest::Client::new();
        let bytes = client
            .get(parsed_url)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to fetch profile picture: {}", e)))?
            .bytes()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to read profile picture bytes: {}", e)))?;

        tokio::fs::write(&dest, bytes).await?;

        Ok((dest, filename))
    }
}
