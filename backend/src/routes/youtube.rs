use axum::extract::State;
use axum::response::Response;
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::config::AppConfig;
use crate::error::AppError;
use crate::services::youtube::YouTubeService;
use crate::utils::{
    clean_yt_url, is_valid_audio_format, is_valid_audio_quality, is_valid_lang_code,
    is_valid_resolution, is_valid_time_format, is_valid_video_format, is_valid_youtube_url,
};

#[derive(Deserialize)]
pub struct VideoRequest {
    pub url: String,
    #[serde(default = "default_resolution")]
    pub resolution: String,
    #[serde(default = "default_format")]
    pub format: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

fn default_resolution() -> String {
    "best".to_string()
}
fn default_format() -> String {
    "mp4".to_string()
}

#[derive(Deserialize)]
pub struct AudioRequest {
    pub url: String,
    #[serde(default = "default_quality")]
    pub quality: String,
    #[serde(default = "default_audio_format")]
    pub format: String,
}

fn default_quality() -> String {
    "192".to_string()
}
fn default_audio_format() -> String {
    "mp3".to_string()
}

#[derive(Deserialize)]
pub struct SimpleRequest {
    pub url: String,
}

#[derive(Deserialize)]
pub struct SubtitleRequest {
    pub url: String,
    #[serde(default = "default_lang")]
    pub lang: String,
}

fn default_lang() -> String {
    "en".to_string()
}

#[derive(Deserialize)]
pub struct ChannelRequest {
    pub channel_url: String,
}

pub async fn video(
    State(config): State<AppConfig>,
    Json(req): Json<VideoRequest>,
) -> Result<Response, AppError> {
    let raw_url = req.url.trim();
    if !is_valid_youtube_url(raw_url) {
        return Err(AppError::BadRequest("Invalid or unsupported YouTube URL".to_string()));
    }
    if !is_valid_resolution(&req.resolution) {
        return Err(AppError::BadRequest("Invalid video resolution specified".to_string()));
    }
    if !is_valid_video_format(&req.format) {
        return Err(AppError::BadRequest("Invalid video format specified".to_string()));
    }
    if let Some(ref st) = req.start_time {
        if !is_valid_time_format(st) {
            return Err(AppError::BadRequest("Invalid start time format".to_string()));
        }
    }
    if let Some(ref et) = req.end_time {
        if !is_valid_time_format(et) {
            return Err(AppError::BadRequest("Invalid end time format".to_string()));
        }
    }

    let _permit = config.semaphore.acquire().await.map_err(|_| {
        AppError::Internal("Server busy, please retry shortly".to_string())
    })?;

    let url = clean_yt_url(raw_url);
    let (path, filename) = YouTubeService::download_video(
        &config.ytdlp_bin,
        &config.download_dir,
        &url,
        &req.resolution,
        &req.format,
        req.start_time.as_deref(),
        req.end_time.as_deref(),
    )
    .await?;

    crate::routes::stream_file_response(path, filename).await
}

pub async fn audio(
    State(config): State<AppConfig>,
    Json(req): Json<AudioRequest>,
) -> Result<Response, AppError> {
    let raw_url = req.url.trim();
    if !is_valid_youtube_url(raw_url) {
        return Err(AppError::BadRequest("Invalid or unsupported YouTube URL".to_string()));
    }
    if !is_valid_audio_quality(&req.quality) {
        return Err(AppError::BadRequest("Invalid audio quality specified".to_string()));
    }
    if !is_valid_audio_format(&req.format) {
        return Err(AppError::BadRequest("Invalid audio format specified".to_string()));
    }

    let _permit = config.semaphore.acquire().await.map_err(|_| {
        AppError::Internal("Server busy, please retry shortly".to_string())
    })?;

    let url = clean_yt_url(raw_url);
    let (path, filename) = YouTubeService::download_audio(
        &config.ytdlp_bin,
        &config.download_dir,
        &url,
        &req.quality,
        &req.format,
    )
    .await?;

    crate::routes::stream_file_response(path, filename).await
}

pub async fn thumbnail(
    State(config): State<AppConfig>,
    Json(req): Json<SimpleRequest>,
) -> Result<Response, AppError> {
    let raw_url = req.url.trim();
    if !is_valid_youtube_url(raw_url) {
        return Err(AppError::BadRequest("Invalid or unsupported YouTube URL".to_string()));
    }

    let _permit = config.semaphore.acquire().await.map_err(|_| {
        AppError::Internal("Server busy, please retry shortly".to_string())
    })?;

    let url = clean_yt_url(raw_url);
    let (path, filename) = YouTubeService::download_thumbnail(
        &config.ytdlp_bin,
        &config.download_dir,
        &url,
    )
    .await?;

    crate::routes::stream_file_response(path, filename).await
}

pub async fn subtitles(
    State(config): State<AppConfig>,
    Json(req): Json<SubtitleRequest>,
) -> Result<Response, AppError> {
    let raw_url = req.url.trim();
    if !is_valid_youtube_url(raw_url) {
        return Err(AppError::BadRequest("Invalid or unsupported YouTube URL".to_string()));
    }
    if !is_valid_lang_code(&req.lang) {
        return Err(AppError::BadRequest("Invalid language code specified".to_string()));
    }

    let _permit = config.semaphore.acquire().await.map_err(|_| {
        AppError::Internal("Server busy, please retry shortly".to_string())
    })?;

    let url = clean_yt_url(raw_url);
    let (path, filename) = YouTubeService::download_subtitles(
        &config.ytdlp_bin,
        &config.download_dir,
        &url,
        &req.lang,
    )
    .await?;

    crate::routes::stream_file_response(path, filename).await
}

pub async fn channel_art(
    State(config): State<AppConfig>,
    Json(req): Json<ChannelRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let channel_url = req.channel_url.trim();
    if !is_valid_youtube_url(channel_url) {
        return Err(AppError::BadRequest("Invalid YouTube channel URL".to_string()));
    }

    let _permit = config.semaphore.acquire().await.map_err(|_| {
        AppError::Internal("Server busy, please retry shortly".to_string())
    })?;

    let assets = YouTubeService::download_channel_art(
        &config.ytdlp_bin,
        &config.download_dir,
        channel_url,
    )
    .await?;

    Ok(Json(json!({ "assets": assets })))
}
