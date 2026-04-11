"""
/api/youtube — video, audio, thumbnail, subtitles, channel art
"""

import asyncio
import os
from fastapi import APIRouter, HTTPException, BackgroundTasks
from fastapi.responses import FileResponse
from pydantic import BaseModel
from typing import Optional

from utils import clean_yt_url
from services.youtube_service import (
    download_video,
    download_audio,
    download_thumbnail,
    download_subtitles,
    download_channel_art,
)

router = APIRouter()


class VideoRequest(BaseModel):
    url: str
    resolution: str = "best"
    format: str = "mp4"
    start_time: Optional[str] = None
    end_time: Optional[str] = None


class AudioRequest(BaseModel):
    url: str
    quality: str = "192"
    format: str = "mp3"


class SimpleRequest(BaseModel):
    url: str


class SubtitleRequest(BaseModel):
    url: str
    lang: str = "en"


class ChannelRequest(BaseModel):
    channel_url: str   # youtube.com/@handle  or  youtube.com/channel/UCxxxxxx


async def _delete_later(path: str, delay: int = 3600):
    await asyncio.sleep(delay)
    try:
        os.unlink(path)
    except Exception:
        pass


@router.post("/video")
async def video(req: VideoRequest, bg: BackgroundTasks):
    url = clean_yt_url(req.url.strip())
    try:
        path, filename = await download_video(
            url=url,
            resolution=req.resolution,
            fmt=req.format,
            start_time=req.start_time,
            end_time=req.end_time,
        )
    except Exception as e:
        raise HTTPException(500, detail=str(e))
    bg.add_task(_delete_later, path)
    return FileResponse(path, filename=filename, media_type="video/mp4")


@router.post("/audio")
async def audio(req: AudioRequest, bg: BackgroundTasks):
    url = clean_yt_url(req.url.strip())
    try:
        path, filename = await download_audio(url=url, quality=req.quality, fmt=req.format)
    except Exception as e:
        raise HTTPException(500, detail=str(e))
    bg.add_task(_delete_later, path)
    return FileResponse(path, filename=filename, media_type="audio/mpeg")


@router.post("/thumbnail")
async def thumbnail(req: SimpleRequest, bg: BackgroundTasks):
    url = clean_yt_url(req.url.strip())
    try:
        path, filename = await download_thumbnail(url)
    except Exception as e:
        raise HTTPException(500, detail=str(e))
    bg.add_task(_delete_later, path)
    return FileResponse(path, filename=filename, media_type="image/jpeg")


@router.post("/subtitles")
async def subtitles(req: SubtitleRequest, bg: BackgroundTasks):
    url = clean_yt_url(req.url.strip())
    try:
        path, filename = await download_subtitles(url, req.lang)
    except Exception as e:
        raise HTTPException(500, detail=str(e))
    bg.add_task(_delete_later, path)
    return FileResponse(path, filename=filename, media_type="text/plain")


@router.post("/channel-art")
async def channel_art(req: ChannelRequest, bg: BackgroundTasks):
    """
    Downloads avatar (JPG) and banner (JPG) for a YouTube channel.
    Returns JSON — frontend opens each asset URL to trigger download.

    {
      "assets": [
        {"type": "avatar", "filename": "...", "url": "/files/..."},
        {"type": "banner", "filename": "...", "url": "/files/..."}
      ]
    }
    """
    try:
        assets = await download_channel_art(req.channel_url.strip())
    except Exception as e:
        raise HTTPException(500, detail=str(e))

    for asset in assets:
        bg.add_task(_delete_later, f"downloads/{asset['filename']}")

    return {"assets": assets}
