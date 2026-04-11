"""
/api/instagram — post, reel, profile, thumbnail
"""

import asyncio
import os
from fastapi import APIRouter, HTTPException, BackgroundTasks
from fastapi.responses import FileResponse
from pydantic import BaseModel
from typing import Optional

from utils import clean_ig_url, extract_ig_username
from services.instagram_service import (
    download_post,
    download_reel,
    download_profile,
    download_ig_thumbnail,
)

router = APIRouter()


class PostRequest(BaseModel):
    url: str
    format: str = "mp4"
    start_time: Optional[str] = None
    end_time: Optional[str] = None


class ProfileRequest(BaseModel):
    # accepts either a username ("oiupoyt") or full URL ("instagram.com/oiupoyt/")
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
    url = clean_ig_url(req.url.strip())
    try:
        path, filename = await download_post(
            url=url, fmt=req.format,
            start_time=req.start_time, end_time=req.end_time,
        )
    except Exception as e:
        raise HTTPException(500, detail=str(e))
    bg.add_task(_delete_later, path)
    media_type = "video/mp4" if filename.endswith(".mp4") else "image/jpeg"
    return FileResponse(path, filename=filename, media_type=media_type)


@router.post("/reel")
async def reel(req: PostRequest, bg: BackgroundTasks):
    url = clean_ig_url(req.url.strip())
    try:
        path, filename = await download_reel(
            url=url, start_time=req.start_time, end_time=req.end_time,
        )
    except Exception as e:
        raise HTTPException(500, detail=str(e))
    bg.add_task(_delete_later, path)
    return FileResponse(path, filename=filename, media_type="video/mp4")


@router.post("/profile")
async def profile(req: ProfileRequest, bg: BackgroundTasks):
    """
    Accepts a username ("oiupoyt") or full URL ("https://www.instagram.com/oiupoyt/").
    Downloads the profile picture as a JPEG.
    """
    raw = req.username.strip()
    # if it looks like a URL, extract username from it
    if "instagram.com" in raw:
        try:
            username = extract_ig_username(raw)
        except ValueError as e:
            raise HTTPException(400, detail=str(e))
    else:
        username = raw.lstrip("@")

    if not username:
        raise HTTPException(400, detail="no username provided")

    try:
        path, filename = await download_profile(username)
    except Exception as e:
        raise HTTPException(500, detail=str(e))
    bg.add_task(_delete_later, path)
    return FileResponse(path, filename=filename, media_type="image/jpeg")


@router.post("/thumbnail")
async def thumbnail(req: SimpleRequest, bg: BackgroundTasks):
    url = clean_ig_url(req.url.strip())
    try:
        path, filename = await download_ig_thumbnail(url)
    except Exception as e:
        raise HTTPException(500, detail=str(e))
    bg.add_task(_delete_later, path)
    return FileResponse(path, filename=filename, media_type="image/jpeg")
