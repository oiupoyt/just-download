"""
/api/info — platform detection and metadata preview
"""

import re
from fastapi import APIRouter, HTTPException
from pydantic import BaseModel

from utils import clean_yt_url, clean_ig_url, is_ig_profile_url, extract_ig_username
from services.youtube_service import fetch_yt_info
from services.instagram_service import fetch_ig_post_info, fetch_ig_profile_info

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
    url = req.url.strip()
    platform = detect_platform(url)

    try:
        if platform == "youtube":
            url = clean_yt_url(url)
            data = await fetch_yt_info(url)

        elif platform == "instagram":
            url = clean_ig_url(url)
            if is_ig_profile_url(url):
                # profile page — show username + avatar
                username = extract_ig_username(url)
                data = await fetch_ig_profile_info(username)
            else:
                # post / reel / tv
                data = await fetch_ig_post_info(url)
        else:
            raise HTTPException(400, detail="unsupported platform — only YouTube and Instagram")

    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(500, detail=f"fetch failed: {str(e)}")

    return {"platform": platform, **data}
