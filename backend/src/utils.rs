use regex::Regex;
use reqwest::Url;
use std::sync::LazyLock;

static YT_V_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[?&]v=([A-Za-z0-9_-]{11})").expect("valid regex")
});
static YT_SHORT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^https?://youtu\.be/([A-Za-z0-9_-]{11})").expect("valid regex")
});
static TIME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{1,2}:)?\d{1,2}:\d{2}(\.\d+)?$|^\d+(\.\d+)?$").expect("valid regex")
});
static LANG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-zA-Z]{2,10}(-[a-zA-Z0-9]{2,10})?$").expect("valid regex")
});
static IG_USER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-zA-Z0-9._]{1,30}$").expect("valid regex")
});
static SAFE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"[^a-zA-Z0-9._-]"#).expect("valid regex")
});

/// Validates that a string is a safe HTTP/HTTPS URL and returns the parsed URL.
pub fn parse_safe_url(raw: &str) -> Result<Url, &'static str> {
    let raw = raw.trim();
    if raw.len() > 2048 {
        return Err("URL exceeds maximum allowed length");
    }
    if raw.starts_with('-') || raw.contains('\0') || raw.contains('\n') || raw.contains('\r') {
        return Err("Invalid characters in URL");
    }
    let parsed = Url::parse(raw).map_err(|_| "Malformed URL format")?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err("Only HTTP and HTTPS URLs are allowed");
    }
    let host = match parsed.host_str() {
        Some(h) => h.to_ascii_lowercase(),
        None => return Err("URL must have a valid hostname"),
    };

    // Block localhost, IP addresses, private IP ranges (anti-SSRF)
    if host == "localhost"
        || host == "127.0.0.1"
        || host == "0.0.0.0"
        || host == "::1"
        || host.starts_with("192.168.")
        || host.starts_with("10.")
        || host.starts_with("172.")
        || host.starts_with("169.254.")
    {
        return Err("Requests to local or private networks are forbidden");
    }

    Ok(parsed)
}

/// Checks if a URL is a valid, allowed YouTube URL
pub fn is_valid_youtube_url(url: &str) -> bool {
    match parse_safe_url(url) {
        Ok(parsed) => {
            if let Some(host) = parsed.host_str() {
                let host = host.to_ascii_lowercase();
                host == "youtube.com"
                    || host == "www.youtube.com"
                    || host == "m.youtube.com"
                    || host == "music.youtube.com"
                    || host == "youtu.be"
                    || host == "www.youtu.be"
                    || host.ends_with(".youtube.com")
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

/// Checks if a URL is a valid, allowed Instagram URL
pub fn is_valid_instagram_url(url: &str) -> bool {
    match parse_safe_url(url) {
        Ok(parsed) => {
            if let Some(host) = parsed.host_str() {
                let host = host.to_ascii_lowercase();
                host == "instagram.com"
                    || host == "www.instagram.com"
                    || host == "instagr.am"
                    || host == "www.instagr.am"
                    || host.ends_with(".instagram.com")
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

pub fn is_valid_resolution(res: &str) -> bool {
    matches!(
        res,
        "best"
            | "144"
            | "240"
            | "360"
            | "480"
            | "720"
            | "1080"
            | "1440"
            | "2160"
            | "4320"
    )
}

pub fn is_valid_video_format(fmt: &str) -> bool {
    matches!(fmt, "mp4" | "webm" | "mkv")
}

pub fn is_valid_audio_format(fmt: &str) -> bool {
    matches!(fmt, "mp3" | "m4a" | "opus" | "flac" | "wav")
}

pub fn is_valid_audio_quality(quality: &str) -> bool {
    matches!(quality, "0" | "64" | "128" | "192" | "256" | "320")
}

pub fn is_valid_lang_code(lang: &str) -> bool {
    LANG_RE.is_match(lang) && lang.len() <= 15
}

pub fn is_valid_time_format(time: &str) -> bool {
    TIME_RE.is_match(time) && time.len() <= 15
}

pub fn is_valid_ig_username(user: &str) -> bool {
    IG_USER_RE.is_match(user) && !user.starts_with('.') && !user.contains("..")
}

pub fn clean_yt_url(url: &str) -> String {
    let trimmed = url.trim();
    if let Some(caps) = YT_V_RE.captures(trimmed) {
        if let Some(id) = caps.get(1) {
            return format!("https://www.youtube.com/watch?v={}", id.as_str());
        }
    }
    if let Some(caps) = YT_SHORT_RE.captures(trimmed) {
        if let Some(id) = caps.get(1) {
            return format!("https://youtu.be/{}", id.as_str());
        }
    }
    trimmed.to_string()
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

    if username.is_empty() || !is_valid_ig_username(username) {
        Err("could not extract valid username from URL".to_string())
    } else {
        Ok(username.to_string())
    }
}

pub fn safe_filename(name: &str) -> String {
    let sanitized = SAFE_RE.replace_all(name, "_");
    let trimmed = sanitized.trim_matches(|c| c == '.' || c == '_' || c == '-');
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_youtube_urls() {
        assert!(is_valid_youtube_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ"));
        assert!(is_valid_youtube_url("https://youtu.be/dQw4w9WgXcQ"));
        assert!(is_valid_youtube_url("https://m.youtube.com/watch?v=dQw4w9WgXcQ"));
        assert!(is_valid_youtube_url("http://youtube.com/shorts/dQw4w9WgXcQ"));
    }

    #[test]
    fn test_invalid_youtube_urls() {
        assert!(!is_valid_youtube_url("file:///etc/passwd"));
        assert!(!is_valid_youtube_url("http://localhost:8022#youtube.com"));
        assert!(!is_valid_youtube_url("http://127.0.0.1:8000/youtube.com"));
        assert!(!is_valid_youtube_url("http://192.168.0.124:8022#youtube.com"));
        assert!(!is_valid_youtube_url("http://attacker.com/youtube.com"));
        assert!(!is_valid_youtube_url("http://youtube.com.attacker.com"));
        assert!(!is_valid_youtube_url("--exec echo pwned"));
    }

    #[test]
    fn test_valid_instagram_urls() {
        assert!(is_valid_instagram_url("https://www.instagram.com/p/DF123/"));
        assert!(is_valid_instagram_url("https://instagram.com/reel/C890/"));
    }

    #[test]
    fn test_invalid_instagram_urls() {
        assert!(!is_valid_instagram_url("file:///etc/shadow"));
        assert!(!is_valid_instagram_url("http://localhost:3000/instagram.com"));
        assert!(!is_valid_instagram_url("http://instagram.com.evil.com"));
    }

    #[test]
    fn test_resolutions() {
        assert!(is_valid_resolution("1080"));
        assert!(is_valid_resolution("best"));
        assert!(!is_valid_resolution("--exec"));
        assert!(!is_valid_resolution("99999"));
    }

    #[test]
    fn test_formats() {
        assert!(is_valid_video_format("mp4"));
        assert!(is_valid_video_format("webm"));
        assert!(!is_valid_video_format("exe"));
        assert!(!is_valid_video_format("--exec"));
    }

    #[test]
    fn test_safe_filename() {
        assert_eq!(safe_filename("../../etc/passwd"), "etc_passwd");
        assert_eq!(safe_filename("great video [1080p].mp4"), "great_video__1080p_.mp4");
        assert_eq!(safe_filename("..."), "file");
    }
}
