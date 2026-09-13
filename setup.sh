#!/usr/bin/env bash
set -e

echo "==> [1/5] Installing system dependencies (ffmpeg, zram-tools, curl)..."
apt-get update -qq
apt-get install -y -qq ffmpeg zram-tools curl

echo "==> [2/5] Downloading optimized just-download Rust binary..."
curl -sL https://github.com/oiupoyt/just-download/releases/download/v2.0.0/just-download-backend -o /usr/local/bin/just-download-backend
chmod +x /usr/local/bin/just-download-backend

echo "==> [3/5] Downloading standalone yt-dlp binary..."
curl -sL https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o /usr/local/bin/yt-dlp
chmod +x /usr/local/bin/yt-dlp

echo "==> [4/5] Configuring directories & systemd service..."
mkdir -p /var/www/just-download/downloads

cat << 'EOF' > /etc/systemd/system/just-download.service
[Unit]
Description=Just-Download Rust Backend Service
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/var/www/just-download
Environment=HOST=0.0.0.0
Environment=PORT=8000
Environment=DOWNLOAD_DIR=/var/www/just-download/downloads
Environment=MAX_CONCURRENT_DOWNLOADS=2
Environment=MAX_FILE_AGE_SECS=1800
ExecStart=/usr/local/bin/just-download-backend
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
EOF

echo "==> [5/5] Enabling and starting just-download service..."
systemctl daemon-reload
systemctl enable --now just-download
systemctl restart just-download

sleep 2
if systemctl is-active --quiet just-download; then
    echo "============================================================"
    echo "✓ Success: just-download Rust backend is running on port 8000!"
    echo "============================================================"
    curl -s http://localhost:8000/health
    echo ""
else
    echo "❌ Service failed to start. Logs:"
    journalctl -u just-download -n 20 --no-pager
fi
