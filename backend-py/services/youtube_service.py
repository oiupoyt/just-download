"""
YouTube service — all yt-dlp operations.
"""

import asyncio
import re
import uuid
from pathlib import Path
from typing import Optional, Tuple, List

import yt_dlp

DOWNLOAD_DIR = Path("downloads")
DOWNLOAD_DIR.mkdir(exist_ok=True)

_BASE_OPTS = {
    "quiet":       True,
    "no_warnings": True,
    "noprogress":  True,
}


def _run(func, *args, **kwargs):
    loop = asyncio.get_event_loop()
    return loop.run_in_executor(None, lambda: func(*args, **kwargs))


def _uid() -> str:
    return uuid.uuid4().hex[:10]


def _safe(name: str) -> str:
    return re.sub(r'[\\/*?:"<>|]', "_", name or "file")[:80]


def _fmt_duration(seconds) -> str:
    if not seconds:
        return "0:00"
    h, rem = divmod(int(seconds), 3600)
    m, s = divmod(rem, 60)
    return f"{h}:{m:02d}:{s:02d}" if h else f"{m}:{s:02d}"


def _parse_time(t: str) -> float:
    parts = [float(p) for p in t.strip().split(":")]
    if len(parts) == 3:
        return parts[0] * 3600 + parts[1] * 60 + parts[2]
    if len(parts) == 2:
        return parts[0] * 60 + parts[1]
    return parts[0]


def _download_image_url(url: str, dest: Path) -> Path:
    import httpx
    resp = httpx.get(url, follow_redirects=True, timeout=20)
    resp.raise_for_status()
    dest.write_bytes(resp.content)
    return dest


def _first_file(pattern: str) -> Optional[Path]:
    candidates = list(DOWNLOAD_DIR.glob(pattern))
    return candidates[0] if candidates else None


# ── info ──────────────────────────────────────────────────────────────────────

async def fetch_yt_info(url: str) -> dict:
    def _fetch():
        opts = {**_BASE_OPTS, "skip_download": True}
        with yt_dlp.YoutubeDL(opts) as ydl:
            info = ydl.extract_info(url, download=False)
        formats = info.get("formats") or []
        resolutions = sorted(
            {f.get("height") for f in formats if f.get("height")},
            reverse=True,
        )
        fps_options = sorted(
            {int(f.get("fps")) for f in formats if f.get("fps")},
            reverse=True,
        )
        return {
            "title":                 info.get("title"),
            "duration":              info.get("duration"),
            "duration_str":          _fmt_duration(info.get("duration", 0)),
            "thumbnail":             info.get("thumbnail"),
            "uploader":              info.get("uploader"),
            "view_count":            info.get("view_count"),
            "upload_date":           info.get("upload_date"),
            "available_resolutions": resolutions,
            "fps_options":           fps_options,
            "has_subtitles":         bool(info.get("subtitles")),
            "subtitle_langs":        list((info.get("subtitles") or {}).keys()),
        }
    return await _run(_fetch)


# ── video ─────────────────────────────────────────────────────────────────────

async def download_video(
    url: str,
    resolution: str = "best",
    fmt: str = "mp4",
    start_time: Optional[str] = None,
    end_time: Optional[str] = None,
) -> Tuple[str, str]:
    uid      = _uid()
    out_tmpl = str(DOWNLOAD_DIR / f"yt_video_{uid}.%(ext)s")

    if resolution == "best":
        fmt_sel = f"bestvideo[ext={fmt}]+bestaudio/bestvideo+bestaudio/best"
    else:
        fmt_sel = (
            f"bestvideo[height<={resolution}][ext={fmt}]+bestaudio/"
            f"bestvideo[height<={resolution}]+bestaudio/best"
        )

    opts = {
        **_BASE_OPTS,
        "format":              fmt_sel,
        "outtmpl":             out_tmpl,
        "merge_output_format": fmt,
    }

    if start_time or end_time:
        start_s = _parse_time(start_time) if start_time else 0
        end_s   = _parse_time(end_time)   if end_time   else None

        def _range_fn(info_dict, ydl):
            end = end_s if end_s is not None else info_dict.get("duration", 86400)
            return [{"start_time": start_s, "end_time": end}]

        opts["download_ranges"]         = _range_fn
        opts["force_keyframes_at_cuts"] = True

    def _dl():
        with yt_dlp.YoutubeDL(opts) as ydl:
            info  = ydl.extract_info(url)
            title = _safe(info.get("title", "video"))
        final = _first_file(f"yt_video_{uid}.*")
        if not final:
            raise FileNotFoundError("output file not found after download")
        return str(final), f"{title}{final.suffix}"

    return await _run(_dl)


# ── audio ─────────────────────────────────────────────────────────────────────

async def download_audio(
    url: str,
    quality: str = "192",
    fmt: str = "mp3",
) -> Tuple[str, str]:
    uid      = _uid()
    out_tmpl = str(DOWNLOAD_DIR / f"yt_audio_{uid}.%(ext)s")
    codec    = {"mp3": "mp3", "opus": "opus", "flac": "flac", "m4a": "m4a"}.get(fmt, "mp3")

    opts = {
        **_BASE_OPTS,
        "format":  "bestaudio/best",
        "outtmpl": out_tmpl,
        "postprocessors": [
            {"key": "FFmpegExtractAudio", "preferredcodec": codec, "preferredquality": quality},
            {"key": "FFmpegMetadata", "add_metadata": True},
        ],
    }

    def _dl():
        with yt_dlp.YoutubeDL(opts) as ydl:
            info  = ydl.extract_info(url)
            title = _safe(info.get("title", "audio"))
        final = _first_file(f"yt_audio_{uid}.*")
        if not final:
            raise FileNotFoundError("audio file not found after download")
        return str(final), f"{title}{final.suffix}"

    return await _run(_dl)


# ── thumbnail ─────────────────────────────────────────────────────────────────

async def download_thumbnail(url: str) -> Tuple[str, str]:
    uid      = _uid()
    out_tmpl = str(DOWNLOAD_DIR / f"yt_thumb_{uid}.%(ext)s")

    opts = {
        **_BASE_OPTS,
        "skip_download":  True,
        "writethumbnail": True,
        "outtmpl":        out_tmpl,
    }

    def _dl():
        with yt_dlp.YoutubeDL(opts) as ydl:
            info  = ydl.extract_info(url, download=True)
            title = _safe(info.get("title", "thumbnail"))
        for ext in ("jpg", "webp", "png", "jpeg"):
            candidate = DOWNLOAD_DIR / f"yt_thumb_{uid}.{ext}"
            if candidate.exists():
                return str(candidate), f"{title}_thumbnail.{ext}"
        raise FileNotFoundError("thumbnail not found after download")

    return await _run(_dl)


# ── subtitles ─────────────────────────────────────────────────────────────────

async def download_subtitles(url: str, lang: str = "en") -> Tuple[str, str]:
    uid      = _uid()
    out_tmpl = str(DOWNLOAD_DIR / f"yt_subs_{uid}.%(ext)s")

    opts = {
        **_BASE_OPTS,
        "skip_download":     True,
        "writesubtitles":    True,
        "writeautomaticsub": True,
        "subtitleslangs":    [lang],
        "subtitlesformat":   "srt",
        "outtmpl":           out_tmpl,
    }

    def _dl():
        with yt_dlp.YoutubeDL(opts) as ydl:
            info  = ydl.extract_info(url, download=True)
            title = _safe(info.get("title", "subtitles"))
        for ext in ("srt", "vtt", "ass"):
            candidate = DOWNLOAD_DIR / f"yt_subs_{uid}.{lang}.{ext}"
            if candidate.exists():
                return str(candidate), f"{title}.{lang}.{ext}"
        final = _first_file(f"yt_subs_{uid}.*")
        if final:
            return str(final), final.name
        raise FileNotFoundError(f"no subtitles found for lang={lang}")

    return await _run(_dl)


# ── channel art ───────────────────────────────────────────────────────────────

async def download_channel_art(channel_url: str) -> List[dict]:
    """
    Downloads avatar + banner for a YouTube channel as separate JPEGs.

    Approach:
      1. Use yt-dlp to get channel metadata (uploader, thumbnails list)
      2. Parse thumbnails list — YouTube returns avatars (square, ggpht CDN)
         and banners (wide ratio) as separate entries
      3. Download each directly with httpx

    Returns list of {"type", "filename", "url"} dicts.
    """

    def _dl():
        import httpx

        uid = _uid()

        # fetch channel page metadata only — no video list needed
        # DO NOT use extract_flat here, it causes yt-dlp to return
        # video entries instead of channel metadata
        opts = {
            **_BASE_OPTS,
            "skip_download":  True,
            "extract_flat":   False,
            "playlist_items": "0",   # fetch 0 videos — just channel info
        }
        with yt_dlp.YoutubeDL(opts) as ydl:
            info = ydl.extract_info(channel_url, download=False)

        channel_name = _safe(
            info.get("channel")
            or info.get("uploader")
            or info.get("title")
            or "channel"
        )
        thumbnails = info.get("thumbnails") or []
        results    = []

        print(f"[yt channel] found {len(thumbnails)} thumbnails for {channel_name}")
        for t in thumbnails:
            print(f"  w={t.get('width')} h={t.get('height')} url={str(t.get('url',''))[:80]}")

        # ── avatar ────────────────────────────────────────────────────────────
        # YouTube avatar thumbnails:
        #   - hosted on ggpht.com or lh3.googleusercontent.com
        #   - square aspect ratio (width == height)
        avatar_url = None
        avatar_candidates = [
            t for t in thumbnails
            if t.get("url") and _is_avatar_thumb(t)
        ]
        if avatar_candidates:
            # pick highest resolution
            best = max(
                avatar_candidates,
                key=lambda t: (t.get("width") or 0) * (t.get("height") or 0),
            )
            avatar_url = best["url"]
            # for ggpht URLs, request a larger size by tweaking the size param
            avatar_url = _upscale_ggpht(avatar_url, 800)

        # fallback: channel's own thumbnail field
        if not avatar_url and info.get("thumbnail"):
            avatar_url = info["thumbnail"]

        if avatar_url:
            dest = DOWNLOAD_DIR / f"{channel_name}_avatar_{uid}.jpg"
            try:
                _download_image_url(avatar_url, dest)
                results.append({
                    "type":     "avatar",
                    "filename": dest.name,
                    "url":      f"/files/{dest.name}",
                })
                print(f"[yt channel] avatar saved: {dest.name}")
            except Exception as e:
                print(f"[yt channel] avatar download failed: {e}")

        # ── banner ────────────────────────────────────────────────────────────
        # Banners are wide (ratio > 3:1) or explicitly keyed
        banner_url = info.get("header_image") or info.get("banner")

        if not banner_url:
            banner_candidates = [
                t for t in thumbnails
                if t.get("url") and _is_banner_thumb(t)
            ]
            if banner_candidates:
                best = max(banner_candidates, key=lambda t: t.get("width") or 0)
                banner_url = best["url"]

        if banner_url:
            dest = DOWNLOAD_DIR / f"{channel_name}_banner_{uid}.jpg"
            try:
                _download_image_url(banner_url, dest)
                results.append({
                    "type":     "banner",
                    "filename": dest.name,
                    "url":      f"/files/{dest.name}",
                })
                print(f"[yt channel] banner saved: {dest.name}")
            except Exception as e:
                print(f"[yt channel] banner download failed: {e}")

        if not results:
            raise ValueError(
                "no channel art found — use a channel URL like "
                "youtube.com/@channelname or youtube.com/channel/UCxxxxxx"
            )

        return results

    return await _run(_dl)


def _is_avatar_thumb(t: dict) -> bool:
    """True if this thumbnail looks like a channel avatar (square, ggpht CDN)."""
    url = t.get("url") or ""
    w   = t.get("width") or 0
    h   = t.get("height") or 1

    # ggpht / lh3.googleusercontent = Google's avatar CDN
    if any(cdn in url for cdn in ("ggpht.com", "lh3.googleusercontent.com")):
        return True

    # square-ish and small-to-medium (avatars are usually ≤ 900px)
    if w and h and 0.85 < (w / h) < 1.15 and w <= 900:
        return True

    return False


def _is_banner_thumb(t: dict) -> bool:
    """True if this thumbnail looks like a channel banner (very wide)."""
    url = t.get("url") or ""
    w   = t.get("width") or 0
    h   = t.get("height") or 1

    if "banner" in url.lower():
        return True

    # banners are typically 2560×1440 or similar ultra-wide
    if w and h and (w / h) > 3.0:
        return True

    return False


def _upscale_ggpht(url: str, size: int) -> str:
    """
    ggpht URLs end with =s{size} or =s{size}-c.
    Replace with a larger size for higher resolution avatars.
    e.g. =s88-c-k  →  =s800-c-k
    """
    # match =sNNN optionally followed by other params
    upscaled = re.sub(r"=s\d+", f"=s{size}", url)
    if upscaled != url:
        return upscaled
    # if no size param found, just return original
    return url
