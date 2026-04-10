"""
/api/instagram — post, reel, story, profile, thumbnail
"""

import asyncio
import os
from fastapi import APIRouter, HTTPException, BackgroundTasks
from fastapi.responses import FileResponse
from pydantic import BaseModel
from typing import Optional

from services.instagram_service import (
    download_post,
    download_reel,
    download_profile,
    download_ig_thumbnail,
    fetch_ig_info,
)

router = APIRouter()


class PostRequest(BaseModel):
    url: str
    format: str = "mp4"
    start_time: Optional[str] = None
    end_time: Optional[str] = None


class ProfileRequest(BaseModel):
    username: str


class SimpleRequest(BaseModel):
    url: str


async def _delete_later(path: str, delay: int = 3600):
    await asyncio.sleep(delay)
    try:
        os.unlink(path)
    except Exception:
        pass


@router.post("/post")
async def post(req: PostRequest, bg: BackgroundTasks):
    try:
        path, filename = await download_post(
            url=req.url,
            fmt=req.format,
            start_time=req.start_time,
            end_time=req.end_time,
        )
    except Exception as e:
        raise HTTPException(500, detail=str(e))
    bg.add_task(_delete_later, path)
    media_type = "video/mp4" if filename.endswith(".mp4") else "image/jpeg"
    return FileResponse(path, filename=filename, media_type=media_type)


@router.post("/reel")
async def reel(req: PostRequest, bg: BackgroundTasks):
    # reels use yt-dlp directly — more reliable than instaloader for reels
    try:
        path, filename = await download_reel(
            url=req.url,
            start_time=req.start_time,
            end_time=req.end_time,
        )
    except Exception as e:
        raise HTTPException(500, detail=str(e))
    bg.add_task(_delete_later, path)
    return FileResponse(path, filename=filename, media_type="video/mp4")


@router.post("/profile")
async def profile(req: ProfileRequest, bg: BackgroundTasks):
    try:
        path, filename = await download_profile(req.username)
    except Exception as e:
        raise HTTPException(500, detail=str(e))
    bg.add_task(_delete_later, path)
    return FileResponse(path, filename=filename, media_type="image/jpeg")


@router.post("/thumbnail")
async def thumbnail(req: SimpleRequest, bg: BackgroundTasks):
    try:
        path, filename = await download_ig_thumbnail(req.url)
    except Exception as e:
        raise HTTPException(500, detail=str(e))
    bg.add_task(_delete_later, path)
    return FileResponse(path, filename=filename, media_type="image/jpeg")
