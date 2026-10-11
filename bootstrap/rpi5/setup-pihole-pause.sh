#!/bin/bash
# Install the bounded override: pihole-pause GROUP MINUTES REASON (max 240 min, logged, Telegram),
# re-enabled by a 1-minute timer. His 2026-10-11 ask: agents may override everything except porn.
# Usage: scp bootstrap/rpi5/files/pihole-pause* pi-lan:/tmp/ && ssh pi-lan 'bash -s' < bootstrap/rpi5/setup-pihole-pause.sh
set -euo pipefail
sudo install -m 0755 /tmp/pihole-pause /usr/local/bin/pihole-pause
sudo install -m 0644 /tmp/pihole-pause-expire.service /tmp/pihole-pause-expire.timer /etc/systemd/system/
rm -f /tmp/pihole-pause /tmp/pihole-pause-expire.service /tmp/pihole-pause-expire.timer
sudo mkdir -p /var/lib/pihole-pause && sudo chmod 700 /var/lib/pihole-pause
sudo systemctl daemon-reload && sudo systemctl enable --now pihole-pause-expire.timer
echo "--- verify"; sudo pihole-pause status; systemctl is-active pihole-pause-expire.timer
