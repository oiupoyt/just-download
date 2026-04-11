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
    Given a channel URL (youtube.com/@handle or youtube.com/channel/UCxxxxx),
    downloads the avatar and banner as separate JPEG files.

    Returns:
      [
        {"type": "avatar", "filename": "channelname_avatar.jpg", "url": "/files/..."},
        {"type": "banner", "filename": "channelname_banner.jpg", "url": "/files/..."},
      ]
    """

    def _dl():
        import httpx

        uid = _uid()

        # fetch channel metadata — extract_flat avoids fetching all videos
        opts = {
            **_BASE_OPTS,
            "skip_download": True,
            "extract_flat":  True,
        }
        with yt_dlp.YoutubeDL(opts) as ydl:
            info = ydl.extract_info(channel_url, download=False)

        channel_name = _safe(info.get("channel") or info.get("uploader") or "channel")
        thumbnails   = info.get("thumbnails") or []
        results      = []

        # ── avatar ────────────────────────────────────────────────────────────
        # YouTube channel avatars come from ggpht.com and are square
        avatar_url = None

        # first try thumbnails tagged as square / avatar-like
        square = [t for t in thumbnails if _is_avatar(t)]
        if square:
            # highest resolution square thumbnail
            avatar_url = max(square, key=lambda t: (t.get("width") or 0) * (t.get("height") or 0))["url"]

        # fallback: any thumbnail from ggpht (Google's image CDN for avatars)
        if not avatar_url:
            ggpht = [t for t in thumbnails if "ggpht" in (t.get("url") or "")]
            if ggpht:
                avatar_url = ggpht[-1]["url"]

        # last resort: channel thumbnail field
        if not avatar_url:
            avatar_url = info.get("thumbnail")

        if avatar_url:
            dest = DOWNLOAD_DIR / f"{channel_name}_avatar_{uid}.jpg"
            try:
                _download_image_url(avatar_url, dest)
                results.append({
                    "type":     "avatar",
                    "filename": dest.name,
                    "url":      f"/files/{dest.name}",
                })
            except Exception as e:
                print(f"[yt channel] avatar download failed: {e}")

        # ── banner ────────────────────────────────────────────────────────────
        banner_url = info.get("header_image") or info.get("banner")

        if not banner_url:
            # look for very wide thumbnails (banner aspect ratio)
            wide = [t for t in thumbnails if _is_banner(t)]
            if wide:
                banner_url = max(wide, key=lambda t: t.get("width") or 0)["url"]

        if banner_url:
            dest = DOWNLOAD_DIR / f"{channel_name}_banner_{uid}.jpg"
            try:
                _download_image_url(banner_url, dest)
                results.append({
                    "type":     "banner",
                    "filename": dest.name,
                    "url":      f"/files/{dest.name}",
                })
            except Exception as e:
                print(f"[yt channel] banner download failed: {e}")

        if not results:
            raise ValueError(
                "no channel art found — make sure you're using a channel URL "
                "like youtube.com/@channelname or youtube.com/channel/UCxxxxxx"
            )

        return results

    return await _run(_dl)


def _is_avatar(t: dict) -> bool:
    w   = t.get("width") or 0
    h   = t.get("height") or 1
    url = t.get("url") or ""
    if w and h and 0.8 < (w / h) < 1.3:
        return True
    return any(kw in url for kw in ("ggpht", "photo", "avatar"))


def _is_banner(t: dict) -> bool:
    w   = t.get("width") or 0
    h   = t.get("height") or 1
    url = t.get("url") or ""
    if w and h and (w / h) > 2.5:
        return True
    return "banner" in url.lower()
