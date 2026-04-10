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


def _download_image(url: str, dest: Path) -> Path:
    import httpx
    resp = httpx.get(url, follow_redirects=True, timeout=20)
    resp.raise_for_status()
    dest.write_bytes(resp.content)
    return dest


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
        candidates = list(DOWNLOAD_DIR.glob(f"yt_video_{uid}.*"))
        if not candidates:
            raise FileNotFoundError("output file not found after download")
        final = candidates[0]
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

    codec_map = {"mp3": "mp3", "opus": "opus", "flac": "flac", "m4a": "m4a"}
    codec = codec_map.get(fmt, "mp3")

    opts = {
        **_BASE_OPTS,
        "format":  "bestaudio/best",
        "outtmpl": out_tmpl,
        "postprocessors": [
            {
                "key":              "FFmpegExtractAudio",
                "preferredcodec":   codec,
                "preferredquality": quality,
            },
            {"key": "FFmpegMetadata", "add_metadata": True},
        ],
    }

    def _dl():
        with yt_dlp.YoutubeDL(opts) as ydl:
            info  = ydl.extract_info(url)
            title = _safe(info.get("title", "audio"))
        candidates = list(DOWNLOAD_DIR.glob(f"yt_audio_{uid}.*"))
        if not candidates:
            raise FileNotFoundError("audio file not found after download")
        final = candidates[0]
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
        candidates = list(DOWNLOAD_DIR.glob(f"yt_subs_{uid}.*"))
        if candidates:
            return str(candidates[0]), candidates[0].name
        raise FileNotFoundError(
            f"no subtitles found for lang={lang} — video may not have them"
        )

    return await _run(_dl)


# ── channel art ───────────────────────────────────────────────────────────────

async def download_channel_art(channel_url: str) -> List[dict]:
    """
    Downloads channel avatar and banner as separate JPEG files.
    Returns list of {"type", "filename", "url"} dicts.
    Avatar and banner are identified by aspect ratio heuristics.
    """

    def _dl():
        uid = _uid()
        results = []

        opts = {
            **_BASE_OPTS,
            "skip_download": True,
            "extract_flat":  True,
        }
        with yt_dlp.YoutubeDL(opts) as ydl:
            info = ydl.extract_info(channel_url, download=False)

        thumbnails = info.get("thumbnails") or []

        # ── avatar ────────────────────────────────────────────────────────────
        # avatars are square-ish; YouTube avatar thumbnails often come from
        # ggpht.com or have "photo" in the URL
        avatar_url = None
        avatar_candidates = [
            t for t in thumbnails
            if t.get("url") and _is_avatar(t)
        ]
        if avatar_candidates:
            # pick highest resolution among avatar candidates
            avatar_url = max(
                avatar_candidates,
                key=lambda t: (t.get("width") or 0) * (t.get("height") or 0)
            )["url"]
        elif thumbnails:
            # last resort: use the last thumbnail
            avatar_url = thumbnails[-1]["url"]

        if avatar_url:
            path = DOWNLOAD_DIR / f"yt_ch_{uid}_avatar.jpg"
            try:
                _download_image(avatar_url, path)
                results.append({
                    "type":     "avatar",
                    "filename": path.name,
                    "url":      f"/files/{path.name}",
                })
            except Exception as e:
                print(f"[yt] avatar download failed: {e}")

        # ── banner ────────────────────────────────────────────────────────────
        # banners are very wide (ratio > 3:1), or keyed explicitly
        banner_url = (
            info.get("header_image")
            or info.get("banner")
        )
        if not banner_url:
            banner_candidates = [
                t for t in thumbnails
                if t.get("url") and _is_banner(t)
            ]
            if banner_candidates:
                banner_url = max(
                    banner_candidates,
                    key=lambda t: (t.get("width") or 0)
                )["url"]

        if banner_url:
            path = DOWNLOAD_DIR / f"yt_ch_{uid}_banner.jpg"
            try:
                _download_image(banner_url, path)
                results.append({
                    "type":     "banner",
                    "filename": path.name,
                    "url":      f"/files/{path.name}",
                })
            except Exception as e:
                print(f"[yt] banner download failed: {e}")

        if not results:
            raise ValueError(
                "no channel art found — pass a full channel URL like "
                "youtube.com/@channelname or youtube.com/channel/UCxxxxxx"
            )

        return results

    return await _run(_dl)


def _is_avatar(t: dict) -> bool:
    w = t.get("width") or 0
    h = t.get("height") or 1
    url = t.get("url", "")
    if w and h:
        ratio = w / h
        if 0.8 < ratio < 1.3:
            return True
    return any(kw in url for kw in ("ggpht", "photo", "avatar"))


def _is_banner(t: dict) -> bool:
    w = t.get("width") or 0
    h = t.get("height") or 1
    url = t.get("url", "")
    if w and h and (w / h) > 2.5:
        return True
    return "banner" in url.lower()
