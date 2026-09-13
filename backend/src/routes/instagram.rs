use axum::extract::State;
use axum::response::Response;
use axum::Json;
use serde::Deserialize;

use crate::config::AppConfig;
use crate::error::AppError;
use crate::services::instagram::InstagramService;
use crate::utils::{clean_ig_url, extract_ig_username, is_valid_ig_username, is_valid_instagram_url, is_valid_time_format};

#[derive(Deserialize)]
pub struct PostRequest {
    pub url: String,
    #[serde(default = "default_format")]
    pub format: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

fn default_format() -> String {
    "mp4".to_string()
}

#[derive(Deserialize)]
pub struct ProfileRequest {
    pub username: String,
}

#[derive(Deserialize)]
pub struct SimpleRequest {
    pub url: String,
}

pub async fn post(
    State(config): State<AppConfig>,
    Json(req): Json<PostRequest>,
) -> Result<Response, AppError> {
    let raw_url = req.url.trim();
    if !is_valid_instagram_url(raw_url) {
        return Err(AppError::BadRequest("Invalid or unsupported Instagram URL".to_string()));
    }
    if req.format != "mp4" {
        return Err(AppError::BadRequest("Invalid format for Instagram video — only mp4 is supported".to_string()));
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

    let url = clean_ig_url(raw_url);
    let (path, filename) = InstagramService::download_post(
        &config.ytdlp_bin,
        &config.download_dir,
        &url,
        &req.format,
        req.start_time.as_deref(),
        req.end_time.as_deref(),
    )
    .await?;

    crate::routes::stream_file_response(path, filename).await
}

pub async fn reel(
    State(config): State<AppConfig>,
    Json(req): Json<PostRequest>,
) -> Result<Response, AppError> {
    let raw_url = req.url.trim();
    if !is_valid_instagram_url(raw_url) {
        return Err(AppError::BadRequest("Invalid or unsupported Instagram URL".to_string()));
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

    let url = clean_ig_url(raw_url);
    let (path, filename) = InstagramService::download_reel(
        &config.ytdlp_bin,
        &config.download_dir,
        &url,
        req.start_time.as_deref(),
        req.end_time.as_deref(),
    )
    .await?;

    crate::routes::stream_file_response(path, filename).await
}

pub async fn profile(
    State(config): State<AppConfig>,
    Json(req): Json<ProfileRequest>,
) -> Result<Response, AppError> {
    let raw = req.username.trim();
    let username = if raw.contains("instagram.com") {
        extract_ig_username(raw).map_err(|e| AppError::BadRequest(e))?
    } else {
        raw.trim_start_matches('@').to_string()
    };

    if username.is_empty() || !is_valid_ig_username(&username) {
        return Err(AppError::BadRequest("Invalid Instagram username provided".to_string()));
    }

    let _permit = config.semaphore.acquire().await.map_err(|_| {
        AppError::Internal("Server busy, please retry shortly".to_string())
    })?;

    let (path, filename) = InstagramService::download_profile(
        &config.ytdlp_bin,
        &config.download_dir,
        &username,
    )
    .await?;

    crate::routes::stream_file_response(path, filename).await
}

pub async fn thumbnail(
    State(config): State<AppConfig>,
    Json(req): Json<SimpleRequest>,
) -> Result<Response, AppError> {
    let raw_url = req.url.trim();
    if !is_valid_instagram_url(raw_url) {
        return Err(AppError::BadRequest("Invalid or unsupported Instagram URL".to_string()));
    }

    let _permit = config.semaphore.acquire().await.map_err(|_| {
        AppError::Internal("Server busy, please retry shortly".to_string())
    })?;

    let url = clean_ig_url(raw_url);
    let (path, filename) = InstagramService::download_thumbnail(
        &config.ytdlp_bin,
        &config.download_dir,
        &url,
    )
    .await?;

    crate::routes::stream_file_response(path, filename).await
}
