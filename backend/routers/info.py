"""
/api/info — platform detection and metadata preview
"""

import re
from fastapi import APIRouter, HTTPException
from pydantic import BaseModel

from services.youtube_service import fetch_yt_info
from services.instagram_service import fetch_ig_info

router = APIRouter()

YT_RE = re.compile(r"(youtube\.com|youtu\.be)")
IG_RE = re.compile(r"(instagram\.com|instagr\.am)")


class InfoRequest(BaseModel):
    url: str


def detect_platform(url: str) -> str:
    if YT_RE.search(url):
        return "youtube"
    if IG_RE.search(url):
        return "instagram"
    return "unknown"


@router.post("/detect")
async def detect(req: InfoRequest):
    platform = detect_platform(req.url)
    if platform == "unknown":
        raise HTTPException(400, detail="could not detect platform from that url")
    return {"platform": platform, "url": req.url}


@router.post("/preview")
async def preview(req: InfoRequest):
    platform = detect_platform(req.url)
    try:
        if platform == "youtube":
            data = await fetch_yt_info(req.url)
        elif platform == "instagram":
            data = await fetch_ig_info(req.url)
        else:
            raise HTTPException(400, detail="unsupported platform — only YouTube and Instagram")
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(500, detail=f"fetch failed: {str(e)}")
    return {"platform": platform, **data}
