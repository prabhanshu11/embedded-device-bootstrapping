#!/bin/bash
# Set up Filebrowser web UI for NAS
#
# Serves /mnt/nas on port 8080 as a web file manager.
#
# Usage: ssh pi@<IP> 'bash -s' < bootstrap/rpi5/setup-filebrowser.sh
#
# Runs ON the Pi (not on host).

set -e

echo "=== RPi5 Filebrowser Setup ==="
echo ""

# === Install filebrowser ===
echo "[1/3] Installing filebrowser..."
if [[ -f /usr/local/bin/filebrowser ]]; then
    echo "  - Filebrowser already installed"
    /usr/local/bin/filebrowser version
else
    curl -fsSL https://raw.githubusercontent.com/filebrowser/get/master/get.sh | sudo bash
fi

# === Create config directory ===
echo "[2/3] Setting up config..."
mkdir -p /home/pi/.config/filebrowser

# === Install systemd service ===
echo "[3/3] Installing systemd service..."
sudo tee /etc/systemd/system/filebrowser.service > /dev/null << 'SERVICE'
[Unit]
Description=Filebrowser
After=network.target

[Service]
ExecStart=/usr/local/bin/filebrowser -a 0.0.0.0 -p 8080 -r /mnt/nas -d /home/pi/.config/filebrowser/filebrowser.db
WorkingDirectory=/home/pi
Restart=always
User=pi

[Install]
WantedBy=multi-user.target
SERVICE

sudo systemctl daemon-reload
sudo systemctl enable filebrowser
sudo systemctl restart filebrowser

echo ""
echo "=== Filebrowser Setup Complete ==="
echo "  - URL: http://$(hostname -I | awk '{print $1}'):8080"
echo "  - Root: /mnt/nas"
echo "  - DB: /home/pi/.config/filebrowser/filebrowser.db"
echo "  - Default login: admin/admin (change immediately!)"
