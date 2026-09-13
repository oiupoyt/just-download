use crate::error::AppError;
use crate::services::ytdlp::YtDlp;
use crate::utils::{format_duration, safe_filename};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Serialize)]
pub struct YtInfoResponse {
    pub title: Option<String>,
    pub duration: Option<f64>,
    pub duration_str: String,
    pub thumbnail: Option<String>,
    pub uploader: Option<String>,
    pub view_count: Option<u64>,
    pub upload_date: Option<String>,
    pub available_resolutions: Vec<u64>,
    pub fps_options: Vec<u64>,
    pub has_subtitles: bool,
    pub subtitle_langs: Vec<String>,
}

#[derive(Serialize)]
pub struct ChannelAsset {
    #[serde(rename = "type")]
    pub asset_type: String,
    pub filename: String,
    pub url: String,
}

pub struct YouTubeService;

impl YouTubeService {
    pub async fn fetch_info(bin: &str, url: &str) -> Result<YtInfoResponse, AppError> {
        let json = YtDlp::dump_json(bin, url).await?;

        let title = json.get("title").and_then(|v| v.as_str()).map(String::from);
        let duration = json.get("duration").and_then(|v| v.as_f64());
        let duration_str = format_duration(duration);
        let thumbnail = json.get("thumbnail").and_then(|v| v.as_str()).map(String::from);
        let uploader = json.get("uploader").and_then(|v| v.as_str()).map(String::from);
        let view_count = json.get("view_count").and_then(|v| v.as_u64());
        let upload_date = json.get("upload_date").and_then(|v| v.as_str()).map(String::from);

        let mut res_set = BTreeSet::new();
        let mut fps_set = BTreeSet::new();

        if let Some(formats) = json.get("formats").and_then(|v| v.as_array()) {
            for f in formats {
                if let Some(h) = f.get("height").and_then(|v| v.as_u64()) {
                    res_set.insert(h);
                }
                if let Some(fps) = f.get("fps").and_then(|v| v.as_u64()) {
                    fps_set.insert(fps);
                }
            }
        }

        let mut available_resolutions: Vec<u64> = res_set.into_iter().collect();
        available_resolutions.reverse();

        let mut fps_options: Vec<u64> = fps_set.into_iter().collect();
        fps_options.reverse();

        let mut subtitle_langs = Vec::new();
        let mut has_subtitles = false;

        if let Some(subs) = json.get("subtitles").and_then(|v| v.as_object()) {
            has_subtitles = !subs.is_empty();
            for lang in subs.keys() {
                subtitle_langs.push(lang.clone());
            }
        }

        Ok(YtInfoResponse {
            title,
            duration,
            duration_str,
            thumbnail,
            uploader,
            view_count,
            upload_date,
            available_resolutions,
            fps_options,
            has_subtitles,
            subtitle_langs,
        })
    }

    pub async fn download_video(
        bin: &str,
        download_dir: &Path,
        url: &str,
        resolution: &str,
        fmt: &str,
        start_time: Option<&str>,
        end_time: Option<&str>,
    ) -> Result<(PathBuf, String), AppError> {
        let uid = &Uuid::new_v4().to_string()[..10];
        let out_prefix = format!("yt_video_{}", uid);
        let out_tmpl = format!("{}/{}.%(ext)s", download_dir.display(), out_prefix);

        let fmt_sel = if resolution == "best" {
            format!("bestvideo[ext={}]+bestaudio/bestvideo+bestaudio/best", fmt)
        } else {
            format!(
                "bestvideo[height<={}][ext={}]+bestaudio/bestvideo[height<={}]+bestaudio/best",
                resolution, fmt, resolution
            )
        };

        let mut args: Vec<&str> = vec![
            "-f", &fmt_sel,
            "--merge-output-format", fmt,
            "-o", &out_tmpl,
        ];

        let start_owned;
        let end_owned;
        if let Some(st) = start_time {
            if crate::utils::is_valid_time_format(st) {
                start_owned = format!("*{}", st);
                args.push("--download-sections");
                args.push(&start_owned);
            }
        } else if let Some(et) = end_time {
            if crate::utils::is_valid_time_format(et) {
                end_owned = format!("*-{}", et);
                args.push("--download-sections");
                args.push(&end_owned);
            }
        }

        YtDlp::execute(bin, &args, url).await?;

        let final_file = YtDlp::find_file_with_prefix(download_dir, &out_prefix)
            .ok_or_else(|| AppError::NotFound("Output video file not found".to_string()))?;

        // Extract video title safely
        let title = match YtDlp::dump_json(bin, url).await {
            Ok(json) => safe_filename(json.get("title").and_then(|v| v.as_str()).unwrap_or("video")),
            Err(_) => "video".to_string(),
        };

        let ext = final_file.extension().and_then(|e| e.to_str()).unwrap_or(fmt);
        let download_name = format!("{}.{}", title, ext);

        Ok((final_file, download_name))
    }

    pub async fn download_audio(
        bin: &str,
        download_dir: &Path,
        url: &str,
        quality: &str,
        fmt: &str,
    ) -> Result<(PathBuf, String), AppError> {
        let uid = &Uuid::new_v4().to_string()[..10];
        let out_prefix = format!("yt_audio_{}", uid);
        let out_tmpl = format!("{}/{}.%(ext)s", download_dir.display(), out_prefix);

        let codec = match fmt {
            "opus" => "opus",
            "flac" => "flac",
            "m4a" => "m4a",
            "wav" => "wav",
            _ => "mp3",
        };

        let args = [
            "-x",
            "--audio-format", codec,
            "--audio-quality", quality,
            "-o", &out_tmpl,
        ];

        YtDlp::execute(bin, &args, url).await?;

        let final_file = YtDlp::find_file_with_prefix(download_dir, &out_prefix)
            .ok_or_else(|| AppError::NotFound("Audio file not found".to_string()))?;

        let title = match YtDlp::dump_json(bin, url).await {
            Ok(json) => safe_filename(json.get("title").and_then(|v| v.as_str()).unwrap_or("audio")),
            Err(_) => "audio".to_string(),
        };

        let ext = final_file.extension().and_then(|e| e.to_str()).unwrap_or(codec);
        let download_name = format!("{}.{}", title, ext);

        Ok((final_file, download_name))
    }

    pub async fn download_thumbnail(
        bin: &str,
        download_dir: &Path,
        url: &str,
    ) -> Result<(PathBuf, String), AppError> {
        let uid = &Uuid::new_v4().to_string()[..10];
        let out_prefix = format!("yt_thumb_{}", uid);
        let out_tmpl = format!("{}/{}.%(ext)s", download_dir.display(), out_prefix);

        let args = [
            "--skip-download",
            "--write-thumbnail",
            "-o", &out_tmpl,
        ];

        YtDlp::execute(bin, &args, url).await?;

        let final_file = YtDlp::find_file_with_prefix(download_dir, &out_prefix)
            .ok_or_else(|| AppError::NotFound("Thumbnail file not found".to_string()))?;

        let title = match YtDlp::dump_json(bin, url).await {
            Ok(json) => safe_filename(json.get("title").and_then(|v| v.as_str()).unwrap_or("thumbnail")),
            Err(_) => "thumbnail".to_string(),
        };

        let ext = final_file.extension().and_then(|e| e.to_str()).unwrap_or("jpg");
        let download_name = format!("{}_thumbnail.{}", title, ext);

        Ok((final_file, download_name))
    }

    pub async fn download_subtitles(
        bin: &str,
        download_dir: &Path,
        url: &str,
        lang: &str,
    ) -> Result<(PathBuf, String), AppError> {
        let uid = &Uuid::new_v4().to_string()[..10];
        let out_prefix = format!("yt_subs_{}", uid);
        let out_tmpl = format!("{}/{}.%(ext)s", download_dir.display(), out_prefix);

        let args = [
            "--skip-download",
            "--write-subs",
            "--write-auto-subs",
            "--sub-lang", lang,
            "--sub-format", "srt",
            "-o", &out_tmpl,
        ];

        YtDlp::execute(bin, &args, url).await?;

        let final_file = YtDlp::find_file_with_prefix(download_dir, &out_prefix)
            .ok_or_else(|| AppError::NotFound(format!("No subtitles found for lang={}", lang)))?;

        let title = match YtDlp::dump_json(bin, url).await {
            Ok(json) => safe_filename(json.get("title").and_then(|v| v.as_str()).unwrap_or("subtitles")),
            Err(_) => "subtitles".to_string(),
        };

        let download_name = format!("{}.{}.srt", title, lang);

        Ok((final_file, download_name))
    }

    pub async fn download_channel_art(
        bin: &str,
        download_dir: &Path,
        channel_url: &str,
    ) -> Result<Vec<ChannelAsset>, AppError> {
        let uid = &Uuid::new_v4().to_string()[..10];
        let json = YtDlp::dump_json(bin, channel_url).await?;

        let channel_name = safe_filename(
            json.get("channel")
                .or_else(|| json.get("uploader"))
                .or_else(|| json.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or("channel"),
        );

        let mut assets = Vec::new();
        let client = reqwest::Client::new();

        if let Some(thumbnails) = json.get("thumbnails").and_then(|v| v.as_array()) {
            // Find avatar
            let mut avatar_candidates = Vec::new();
            for t in thumbnails {
                if let Some(u) = t.get("url").and_then(|v| v.as_str()) {
                    let w = t.get("width").and_then(|v| v.as_u64()).unwrap_or(0);
                    let h = t.get("height").and_then(|v| v.as_u64()).unwrap_or(0);
                    if w > 0 && w == h {
                        avatar_candidates.push((w, u));
                    }
                }
            }

            if let Some((_, best_avatar_url)) = avatar_candidates.into_iter().max_by_key(|c| c.0) {
                // Ensure avatar URL is valid safe HTTPS URL
                if let Ok(parsed_url) = crate::utils::parse_safe_url(best_avatar_url) {
                    let filename = format!("{}_avatar_{}.jpg", channel_name, uid);
                    let dest = download_dir.join(&filename);
                    if let Ok(resp) = client.get(parsed_url).send().await {
                        if let Ok(bytes) = resp.bytes().await {
                            let _ = tokio::fs::write(&dest, bytes).await;
                            assets.push(ChannelAsset {
                                asset_type: "avatar".to_string(),
                                filename: filename.clone(),
                                url: format!("/files/{}", filename),
                            });
                        }
                    }
                }
            }

            // Find banner
            let mut banner_candidates = Vec::new();
            for t in thumbnails {
                if let Some(u) = t.get("url").and_then(|v| v.as_str()) {
                    let w = t.get("width").and_then(|v| v.as_u64()).unwrap_or(0);
                    let h = t.get("height").and_then(|v| v.as_u64()).unwrap_or(0);
                    if w > 0 && h > 0 && w > h * 2 {
                        banner_candidates.push((w * h, u));
                    }
                }
            }

            if let Some((_, best_banner_url)) = banner_candidates.into_iter().max_by_key(|c| c.0) {
                if let Ok(parsed_url) = crate::utils::parse_safe_url(best_banner_url) {
                    let filename = format!("{}_banner_{}.jpg", channel_name, uid);
                    let dest = download_dir.join(&filename);
                    if let Ok(resp) = client.get(parsed_url).send().await {
                        if let Ok(bytes) = resp.bytes().await {
                            let _ = tokio::fs::write(&dest, bytes).await;
                            assets.push(ChannelAsset {
                                asset_type: "banner".to_string(),
                                filename: filename.clone(),
                                url: format!("/files/{}", filename),
                            });
                        }
                    }
                }
            }
        }

        Ok(assets)
    }
}
