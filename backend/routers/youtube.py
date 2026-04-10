"""
/api/youtube — video, audio, thumbnail, subtitles, channel art
"""

import asyncio
import os
from fastapi import APIRouter, HTTPException, BackgroundTasks
from fastapi.responses import FileResponse
from pydantic import BaseModel
from typing import Optional

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
    channel_url: str


async def _delete_later(path: str, delay: int = 3600):
    await asyncio.sleep(delay)
    try:
        os.unlink(path)
    except Exception:
        pass


@router.post("/video")
async def video(req: VideoRequest, bg: BackgroundTasks):
    try:
        path, filename = await download_video(
            url=req.url,
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
    try:
        path, filename = await download_audio(
            url=req.url,
            quality=req.quality,
            fmt=req.format,
        )
    except Exception as e:
        raise HTTPException(500, detail=str(e))
    bg.add_task(_delete_later, path)
    return FileResponse(path, filename=filename, media_type="audio/mpeg")


@router.post("/thumbnail")
async def thumbnail(req: SimpleRequest, bg: BackgroundTasks):
    try:
        path, filename = await download_thumbnail(req.url)
    except Exception as e:
        raise HTTPException(500, detail=str(e))
    bg.add_task(_delete_later, path)
    return FileResponse(path, filename=filename, media_type="image/jpeg")


@router.post("/subtitles")
async def subtitles(req: SubtitleRequest, bg: BackgroundTasks):
    try:
        path, filename = await download_subtitles(req.url, req.lang)
    except Exception as e:
        raise HTTPException(500, detail=str(e))
    bg.add_task(_delete_later, path)
    return FileResponse(path, filename=filename, media_type="text/plain")


@router.post("/channel-art")
async def channel_art(req: ChannelRequest, bg: BackgroundTasks):
    """
    Returns JSON with download URLs for avatar and banner images.
    Frontend fetches each URL separately to trigger downloads.
    Response: {"assets": [{"type": "avatar", "url": "/files/..."}, ...]}
    """
    try:
        assets = await download_channel_art(req.channel_url)
    except Exception as e:
        raise HTTPException(500, detail=str(e))

    # schedule cleanup for each file
    for asset in assets:
        path = f"downloads/{asset['filename']}"
        bg.add_task(_delete_later, path)

    return {"assets": assets}
