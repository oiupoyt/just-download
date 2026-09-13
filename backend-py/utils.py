"""
Shared URL utilities.
"""

import re


def clean_yt_url(url: str) -> str:
    """
    Strip playlist/radio params from YouTube URLs so yt-dlp
    treats them as single videos.

    youtube.com/watch?v=ABC&list=RD...&start_radio=1
    → youtube.com/watch?v=ABC
    """
    # if it has a video id, keep only that
    m = re.search(r"[?&]v=([A-Za-z0-9_-]{11})", url)
    if m:
        return f"https://www.youtube.com/watch?v={m.group(1)}"
    # youtu.be short links — keep as-is (no query needed)
    m2 = re.match(r"https?://youtu\.be/([A-Za-z0-9_-]{11})", url)
    if m2:
        return f"https://youtu.be/{m2.group(1)}"
    # channel or playlist URL — return as-is
    return url


def clean_ig_url(url: str) -> str:
    """Strip query params and trailing slashes from Instagram URLs."""
    url = url.split("?")[0].rstrip("/")
    return url


def is_ig_profile_url(url: str) -> bool:
    """
    Returns True if the URL points to a profile page rather than a post/reel.
    Profile: instagram.com/username
    Post:    instagram.com/p/ABC  instagram.com/reel/ABC
    """
    path = re.sub(r"https?://(www\.)?instagram\.com/?", "", url).lstrip("/")
    first_segment = path.split("/")[0].split("?")[0].lstrip("@")
    return first_segment not in ("p", "reel", "reels", "tv", "explore", "stories", "accounts", "")


def extract_ig_username(url: str) -> str:
    """Pull username from instagram.com/username or instagram.com/@username."""
    path = re.sub(r"https?://(www\.)?instagram\.com/?", "", url).lstrip("/")
    username = path.split("/")[0].split("?")[0].lstrip("@").strip()
    if not username:
        raise ValueError("could not extract username from URL")
    return username


def extract_ig_shortcode(url: str) -> str:
    """Pull shortcode from post/reel/tv URLs."""
    m = re.search(r"/(?:p|reel|reels|tv)/([A-Za-z0-9_-]+)", url)
    if m:
        return m.group(1)
    raise ValueError(
        f"could not extract shortcode from: {url} — "
        "expected a post, reel, or tv URL"
    )
