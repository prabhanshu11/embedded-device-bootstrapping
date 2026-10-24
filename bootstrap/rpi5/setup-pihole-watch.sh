#!/bin/bash
# Install pihole-watch (2026-10-23 Pi-hole review: he found all 14 block incidents himself; this tells Telegram).
# Usage: scp bootstrap/rpi5/files/pihole-watch* pi-lan:/tmp/ && ssh pi-lan 'bash -s' < bootstrap/rpi5/setup-pihole-watch.sh
set -euo pipefail
sudo install -m 0755 /tmp/pihole-watch /usr/local/bin/pihole-watch
sudo install -m 0644 /tmp/pihole-watch.service /tmp/pihole-watch.timer /etc/systemd/system/
rm -f /tmp/pihole-watch /tmp/pihole-watch.service /tmp/pihole-watch.timer
sudo mkdir -p /var/lib/pihole-watch && sudo chmod 700 /var/lib/pihole-watch
sudo systemctl daemon-reload && sudo systemctl enable --now pihole-watch.timer
echo "--- verify (dry run over the last 24 h)"; sudo pihole-watch --dry-run --minutes 1440 | head -20
systemctl is-active pihole-watch.timer
