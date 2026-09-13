use regex::Regex;
use std::sync::LazyLock;

static YT_V_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[?&]v=([A-Za-z0-9_-]{11})").expect("valid regex")
});
static YT_SHORT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"https?://youtu\.be/([A-Za-z0-9_-]{11})").expect("valid regex")
});
#[allow(dead_code)]
static IG_SHORTCODE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"/(?:p|reel|reels|tv)/([A-Za-z0-9_-]+)").expect("valid regex")
});
static SAFE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"[\\/*?:"<>|]"#).expect("valid regex")
});

pub fn clean_yt_url(url: &str) -> String {
    if let Some(caps) = YT_V_RE.captures(url) {
        if let Some(id) = caps.get(1) {
            return format!("https://www.youtube.com/watch?v={}", id.as_str());
        }
    }
    if let Some(caps) = YT_SHORT_RE.captures(url) {
        if let Some(id) = caps.get(1) {
            return format!("https://youtu.be/{}", id.as_str());
        }
    }
    url.to_string()
}

pub fn clean_ig_url(url: &str) -> String {
    url.split('?').next().unwrap_or(url).trim_end_matches('/').to_string()
}

pub fn is_ig_profile_url(url: &str) -> bool {
    let stripped = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.")
        .trim_start_matches("instagram.com")
        .trim_start_matches('/');

    let first_segment = stripped
        .split('/')
        .next()
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("")
        .trim_start_matches('@');

    !matches!(
        first_segment,
        "p" | "reel" | "reels" | "tv" | "explore" | "stories" | "accounts" | ""
    )
}

pub fn extract_ig_username(url: &str) -> Result<String, String> {
    let stripped = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.")
        .trim_start_matches("instagram.com")
        .trim_start_matches('/');

    let username = stripped
        .split('/')
        .next()
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("")
        .trim_start_matches('@')
        .trim();

    if username.is_empty() {
        Err("could not extract username from URL".to_string())
    } else {
        Ok(username.to_string())
    }
}

#[allow(dead_code)]
pub fn extract_ig_shortcode(url: &str) -> Result<String, String> {
    if let Some(caps) = IG_SHORTCODE_RE.captures(url) {
        if let Some(sc) = caps.get(1) {
            return Ok(sc.as_str().to_string());
        }
    }
    Err(format!(
        "could not extract shortcode from: {} — expected a post, reel, or tv URL",
        url
    ))
}

pub fn safe_filename(name: &str) -> String {
    let sanitized = SAFE_RE.replace_all(name, "_");
    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        "file".to_string()
    } else if trimmed.len() > 80 {
        trimmed[..80].to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn format_duration(seconds: Option<f64>) -> String {
    match seconds {
        Some(s) if s > 0.0 => {
            let total = s as u64;
            let h = total / 3600;
            let m = (total % 3600) / 60;
            let sec = total % 60;
            if h > 0 {
                format!("{}:{:02}:{:02}", h, m, sec)
            } else {
                format!("{}:{:02}", m, sec)
            }
        }
        _ => "0:00".to_string(),
    }
}

#[allow(dead_code)]
pub fn parse_time_to_seconds(t: &str) -> Option<f64> {
    let parts: Vec<&str> = t.trim().split(':').collect();
    match parts.len() {
        3 => {
            let h = parts[0].parse::<f64>().ok()?;
            let m = parts[1].parse::<f64>().ok()?;
            let s = parts[2].parse::<f64>().ok()?;
            Some(h * 3600.0 + m * 60.0 + s)
        }
        2 => {
            let m = parts[0].parse::<f64>().ok()?;
            let s = parts[1].parse::<f64>().ok()?;
            Some(m * 60.0 + s)
        }
        1 => parts[0].parse::<f64>().ok(),
        _ => None,
    }
}
