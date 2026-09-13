use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::AppConfig;
use crate::error::AppError;
use crate::services::instagram::InstagramService;
use crate::services::youtube::YouTubeService;
use crate::utils::{
    clean_ig_url, clean_yt_url, extract_ig_username, is_ig_profile_url, is_valid_instagram_url,
    is_valid_youtube_url,
};

#[derive(Deserialize)]
pub struct InfoRequest {
    pub url: String,
}

#[derive(Serialize)]
pub struct DetectResponse {
    pub platform: String,
    pub url: String,
}

pub fn detect_platform(url: &str) -> &'static str {
    let trimmed = url.trim();
    if is_valid_youtube_url(trimmed) {
        "youtube"
    } else if is_valid_instagram_url(trimmed) {
        "instagram"
    } else {
        "unknown"
    }
}

pub async fn detect(Json(req): Json<InfoRequest>) -> Result<Json<DetectResponse>, AppError> {
    let platform = detect_platform(&req.url);
    if platform == "unknown" {
        return Err(AppError::BadRequest(
            "Unsupported or invalid URL — only YouTube and Instagram are supported".to_string(),
        ));
    }
    Ok(Json(DetectResponse {
        platform: platform.to_string(),
        url: req.url,
    }))
}

pub async fn preview(
    State(config): State<AppConfig>,
    Json(req): Json<InfoRequest>,
) -> Result<Json<Value>, AppError> {
    let raw_url = req.url.trim();
    let platform = detect_platform(raw_url);

    let mut result_json = match platform {
        "youtube" => {
            let clean = clean_yt_url(raw_url);
            let info = YouTubeService::fetch_info(&config.ytdlp_bin, &clean).await?;
            serde_json::to_value(info)?
        }
        "instagram" => {
            let clean = clean_ig_url(raw_url);
            if is_ig_profile_url(&clean) {
                let username = extract_ig_username(&clean)
                    .map_err(|e| AppError::BadRequest(e))?;
                let info = InstagramService::fetch_profile_info(&config.ytdlp_bin, &username).await?;
                serde_json::to_value(info)?
            } else {
                let info = InstagramService::fetch_post_info(&config.ytdlp_bin, &clean).await?;
                serde_json::to_value(info)?
            }
        }
        _ => {
            return Err(AppError::BadRequest(
                "Unsupported or invalid URL — only YouTube and Instagram are supported".to_string(),
            ));
        }
    };

    if let Some(obj) = result_json.as_object_mut() {
        obj.insert("platform".to_string(), json!(platform));
    }

    Ok(Json(result_json))
}
