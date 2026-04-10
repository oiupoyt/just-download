"""
just download stuff lol — FastAPI backend
Run: uvicorn main:app --reload --host 0.0.0.0 --port 8000
"""

import asyncio
import os
from contextlib import asynccontextmanager
from pathlib import Path

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles

from routers import youtube, instagram, info

DOWNLOAD_DIR = Path("downloads")
DOWNLOAD_DIR.mkdir(exist_ok=True)


@asynccontextmanager
async def lifespan(app: FastAPI):
    asyncio.create_task(_cleanup_loop())
    yield


app = FastAPI(
    title="just download stuff lol",
    description="no cap, it just works™",
    version="2.0.0",
    lifespan=lifespan,
)

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

app.include_router(info.router,      prefix="/api/info",      tags=["info"])
app.include_router(youtube.router,   prefix="/api/youtube",   tags=["youtube"])
app.include_router(instagram.router, prefix="/api/instagram", tags=["instagram"])

app.mount("/files", StaticFiles(directory="downloads"), name="files")


@app.get("/")
async def root():
    return {"status": "ok", "message": "just download stuff lol"}


@app.get("/health")
async def health():
    return {"status": "healthy"}


async def _cleanup_loop():
    """Delete files older than 1 hour, runs every 10 minutes."""
    while True:
        await asyncio.sleep(600)
        now = asyncio.get_event_loop().time()
        for f in DOWNLOAD_DIR.iterdir():
            try:
                if f.is_file() and (now - os.path.getmtime(f)) > 3600:
                    f.unlink()
            except Exception:
                pass
