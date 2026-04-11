"""
Instagram service.

Strategy:
  - Post/reel info:   yt-dlp first, instaloader fallback
  - Post download:    yt-dlp first, instaloader fallback
  - Reel download:    yt-dlp only (instaloader 403s on reels)
  - Profile picture:  yt-dlp (avoids instaloader rate limits on Profile.from_username)
  - Thumbnail:        yt-dlp

Session setup (run once on tablet, needed for instaloader fallback):
  python3 -c "
  import instaloader
  L = instaloader.Instaloader()
  L.interactive_login('your_username')
  L.save_session_to_file('ig_session')
  print('saved')
  "
"""

import asyncio
import os
import re
import shutil
import uuid
from pathlib import Path
from typing import Optional, Tuple

import yt_dlp
import instaloader

DOWNLOAD_DIR = Path("downloads")
DOWNLOAD_DIR.mkdir(exist_ok=True)

SESSION_FILE = Path("ig_session")
IG_USERNAME  = os.getenv("IG_USERNAME", "")
IG_PASSWORD  = os.getenv("IG_PASSWORD", "")

_loader: Optional[instaloader.Instaloader] = None

_BASE_YDL = {
    "quiet":       True,
    "no_warnings": True,
    "noprogress":  True,
}


# ── helpers ───────────────────────────────────────────────────────────────────

def _run(func, *args, **kwargs):
    loop = asyncio.get_event_loop()
    return loop.run_in_executor(None, lambda: func(*args, **kwargs))


def _uid() -> str:
    return uuid.uuid4().hex[:10]


def _safe(name: str) -> str:
    return re.sub(r'[\\/*?:"<>|]', "_", name or "file")[:80]


def _fmt_s(s) -> str:
    if not s:
        return "0:00"
    m, sec = divmod(int(s), 60)
    return f"{m}:{sec:02d}"


def _download_image_url(url: str, dest: Path) -> Path:
    import httpx
    resp = httpx.get(url, follow_redirects=True, timeout=20)
    resp.raise_for_status()
    dest.write_bytes(resp.content)
    return dest


def _trim_video(src: Path, dest: Path, start: Optional[str], end: Optional[str]) -> Path:
    import subprocess
    cmd = ["ffmpeg", "-y"]
    if start:
        cmd += ["-ss", start]
    cmd += ["-i", str(src)]
    if end:
        cmd += ["-to", end]
    cmd += ["-c", "copy", str(dest)]
    result = subprocess.run(cmd, capture_output=True, timeout=120)
    if result.returncode != 0:
        raise RuntimeError(f"ffmpeg trim failed: {result.stderr.decode()[:200]}")
    return dest


def _first_media(directory: Path) -> Optional[Path]:
    return next(
        (f for f in sorted(directory.iterdir())
         if f.suffix in (".mp4", ".jpg", ".jpeg", ".png")),
        None,
    )


def _get_loader() -> instaloader.Instaloader:
    global _loader
    if _loader is not None:
        return _loader

    L = instaloader.Instaloader(
        download_pictures=True,
        download_videos=True,
        download_video_thumbnails=False,
        save_metadata=False,
        post_metadata_txt_pattern="",
        quiet=True,
    )

    if SESSION_FILE.exists():
        try:
            L.load_session_from_file(IG_USERNAME or "user", str(SESSION_FILE))
            print("[ig] session loaded from file")
            _loader = L
            return _loader
        except Exception as e:
            print(f"[ig] session file failed: {e}")

    if IG_USERNAME and IG_PASSWORD:
        try:
            L.login(IG_USERNAME, IG_PASSWORD)
            print("[ig] logged in with password")
            _loader = L
            return _loader
        except Exception as e:
            print(f"[ig] password login failed: {e}")

    print("[ig] WARNING: unauthenticated — most IG content will fail")
    _loader = L
    return _loader


# ── post info ─────────────────────────────────────────────────────────────────

async def fetch_ig_post_info(url: str) -> dict:
    def _via_ytdlp():
        opts = {**_BASE_YDL, "skip_download": True}
        with yt_dlp.YoutubeDL(opts) as ydl:
            info = ydl.extract_info(url, download=False)
        return {
            "type":         "video",
            "title":        info.get("title") or info.get("uploader") or "Instagram post",
            "thumbnail":    info.get("thumbnail"),
            "duration":     info.get("duration"),
            "duration_str": _fmt_s(info.get("duration")),
            "owner":        info.get("uploader"),
            "likes":        info.get("like_count"),
        }

    def _via_instaloader():
        from utils import extract_ig_shortcode
        shortcode = extract_ig_shortcode(url)
        L    = _get_loader()
        post = instaloader.Post.from_shortcode(L.context, shortcode)
        return {
            "type":         "video" if post.is_video else "image",
            "title":        f"@{post.owner_username}",
            "thumbnail":    post.url,
            "duration":     post.video_duration if post.is_video else None,
            "duration_str": _fmt_s(post.video_duration) if post.is_video else None,
            "owner":        post.owner_username,
            "likes":        post.likes,
        }

    try:
        return await _run(_via_ytdlp)
    except Exception:
        pass
    return await _run(_via_instaloader)


# ── profile info ──────────────────────────────────────────────────────────────

async def fetch_ig_profile_info(username: str) -> dict:
    """
    Fetch profile info using yt-dlp's Instagram user extractor.
    This avoids instaloader's Profile.from_username() which gets
    rate-limited and returns "does not exist" even for real accounts.
    """
    def _via_ytdlp():
        # yt-dlp can extract Instagram user pages
        profile_url = f"https://www.instagram.com/{username}/"
        opts = {
            **_BASE_YDL,
            "skip_download": True,
            "extract_flat":  True,
            "playlist_items": "0",
        }
        with yt_dlp.YoutubeDL(opts) as ydl:
            info = ydl.extract_info(profile_url, download=False)

        # yt-dlp returns uploader_url, thumbnail from profile pages
        thumb = info.get("thumbnail") or (
            info.get("thumbnails") or [{}]
        )[-1].get("url")

        return {
            "type":      "profile",
            "title":     f"@{username}",
            "thumbnail": thumb,
            "owner":     username,
            "likes":     info.get("channel_follower_count") or 0,
        }

    def _via_instaloader():
        L       = _get_loader()
        profile = instaloader.Profile.from_username(L.context, username)
        try:
            pic_url = profile.profile_pic_url_no_iphone
        except Exception:
            pic_url = profile.profile_pic_url
        return {
            "type":      "profile",
            "title":     f"@{profile.username}",
            "thumbnail": pic_url,
            "owner":     profile.username,
            "likes":     profile.followers,
        }

    try:
        return await _run(_via_ytdlp)
    except Exception:
        pass
    # instaloader as last resort
    return await _run(_via_instaloader)


# ── post download ─────────────────────────────────────────────────────────────

async def download_post(
    url: str,
    fmt: str = "mp4",
    start_time: Optional[str] = None,
    end_time: Optional[str] = None,
) -> Tuple[str, str]:
    uid = _uid()

    def _via_ytdlp():
        out_tmpl = str(DOWNLOAD_DIR / f"ig_post_{uid}.%(ext)s")
        opts = {
            **_BASE_YDL,
            "format":              "bestvideo+bestaudio/best",
            "outtmpl":             out_tmpl,
            "merge_output_format": "mp4",
        }
        with yt_dlp.YoutubeDL(opts) as ydl:
            info  = ydl.extract_info(url)
            owner = _safe(info.get("uploader") or "ig")
        candidates = list(DOWNLOAD_DIR.glob(f"ig_post_{uid}.*"))
        if not candidates:
            raise FileNotFoundError("yt-dlp produced no file")
        final = candidates[0]
        if (start_time or end_time) and final.suffix == ".mp4":
            trimmed = DOWNLOAD_DIR / f"ig_post_{uid}_trim.mp4"
            _trim_video(final, trimmed, start_time, end_time)
            final.unlink(missing_ok=True)
            final = trimmed
        return str(final), f"{owner}_{uid}{final.suffix}"

    def _via_instaloader():
        from utils import extract_ig_shortcode
        tmp_dir = DOWNLOAD_DIR / f"ig_il_{uid}"
        tmp_dir.mkdir(exist_ok=True)
        try:
            shortcode = extract_ig_shortcode(url)
            L    = _get_loader()
            post = instaloader.Post.from_shortcode(L.context, shortcode)
            L.dirname_pattern = str(tmp_dir)
            L.download_post(post, target=str(tmp_dir))
            media = _first_media(tmp_dir)
            if not media:
                raise FileNotFoundError("instaloader produced no media file")
            owner = _safe(post.owner_username)
            dest  = DOWNLOAD_DIR / f"ig_post_{uid}{media.suffix}"
            if post.is_video and (start_time or end_time):
                _trim_video(media, dest, start_time, end_time)
            else:
                shutil.copy2(media, dest)
            return str(dest), f"{owner}_{uid}{media.suffix}"
        finally:
            shutil.rmtree(tmp_dir, ignore_errors=True)

    try:
        return await _run(_via_ytdlp)
    except Exception:
        pass
    return await _run(_via_instaloader)


# ── reel download ─────────────────────────────────────────────────────────────

async def download_reel(
    url: str,
    start_time: Optional[str] = None,
    end_time: Optional[str] = None,
) -> Tuple[str, str]:
    uid      = _uid()
    out_tmpl = str(DOWNLOAD_DIR / f"ig_reel_{uid}.%(ext)s")

    opts = {
        **_BASE_YDL,
        "format":              "bestvideo+bestaudio/best",
        "outtmpl":             out_tmpl,
        "merge_output_format": "mp4",
    }

    def _dl():
        with yt_dlp.YoutubeDL(opts) as ydl:
            info  = ydl.extract_info(url)
            owner = _safe(info.get("uploader") or "ig")
        candidates = list(DOWNLOAD_DIR.glob(f"ig_reel_{uid}.*"))
        if not candidates:
            raise FileNotFoundError("yt-dlp produced no reel file")
        final = candidates[0]
        if start_time or end_time:
            trimmed = DOWNLOAD_DIR / f"ig_reel_{uid}_trim.mp4"
            _trim_video(final, trimmed, start_time, end_time)
            final.unlink(missing_ok=True)
            final = trimmed
        return str(final), f"{owner}_reel_{uid}.mp4"

    return await _run(_dl)


# ── profile picture download ──────────────────────────────────────────────────

async def download_profile(username: str) -> Tuple[str, str]:
    """
    Downloads profile picture using yt-dlp's Instagram extractor,
    falling back to instaloader if yt-dlp can't find a usable image URL.
    """
    uid = _uid()

    def _via_ytdlp():
        profile_url = f"https://www.instagram.com/{username}/"
        opts = {
            **_BASE_YDL,
            "skip_download":  True,
            "extract_flat":   True,
            "playlist_items": "0",
        }
        with yt_dlp.YoutubeDL(opts) as ydl:
            info = ydl.extract_info(profile_url, download=False)

        # try thumbnail fields in order of preference
        thumb = (
            info.get("thumbnail")
            or next(
                (t.get("url") for t in reversed(info.get("thumbnails") or [])
                 if t.get("url")),
                None,
            )
        )
        if not thumb:
            raise ValueError("yt-dlp found no thumbnail for this profile")

        path = DOWNLOAD_DIR / f"ig_pfp_{_safe(username)}_{uid}.jpg"
        _download_image_url(thumb, path)
        return str(path), f"{_safe(username)}_profile.jpg"

    def _via_instaloader():
        L       = _get_loader()
        profile = instaloader.Profile.from_username(L.context, username)
        try:
            pic_url = profile.profile_pic_url_no_iphone
        except Exception:
            pic_url = profile.profile_pic_url
        path = DOWNLOAD_DIR / f"ig_pfp_{_safe(username)}_{uid}.jpg"
        _download_image_url(pic_url, path)
        return str(path), f"{_safe(username)}_profile.jpg"

    try:
        return await _run(_via_ytdlp)
    except Exception:
        pass
    return await _run(_via_instaloader)


# ── thumbnail ─────────────────────────────────────────────────────────────────

async def download_ig_thumbnail(url: str) -> Tuple[str, str]:
    uid = _uid()

    def _dl():
        opts = {**_BASE_YDL, "skip_download": True}
        with yt_dlp.YoutubeDL(opts) as ydl:
            info = ydl.extract_info(url, download=False)
        thumb_url = info.get("thumbnail")
        if not thumb_url:
            raise ValueError("no thumbnail found for this post")
        owner = _safe(info.get("uploader") or "ig")
        path  = DOWNLOAD_DIR / f"ig_thumb_{uid}.jpg"
        _download_image_url(thumb_url, path)
        return str(path), f"{owner}_thumbnail_{uid}.jpg"

    return await _run(_dl)
