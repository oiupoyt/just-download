# just-download

yeah

↗ [jd.oiupoyt.space](https://jd.oiupoyt.space)

## features

- youtube downloads (video, audio, thumbnail, subtitles, channel art)
- instagram downloads (reels, posts, profile avatars, thumbnails)
- minimal dark web interface
- ultra-fast, low-memory Rust backend (Axum + Tokio)
- stream piping for flat memory footprint (~8MB idle RAM)
- automatic background cleanup of temporary downloads
- concurrency limiter to safeguard low-resource servers

## backend (rust)

```bash
cd backend

# run in development
cargo run

# or run optimized release
cargo run --release
```

Environment variables:
- `PORT` (default: `8000`)
- `HOST` (default: `0.0.0.0`)
- `MAX_CONCURRENT_DOWNLOADS` (default: `3`)
- `MAX_FILE_AGE_SECS` (default: `3600`)

> Note: Legacy Python FastAPI backend is preserved in `backend-py/`.

## license

MIT
