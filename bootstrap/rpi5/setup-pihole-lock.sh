#!/bin/bash
# Lock the Pi-hole admin behind a 6-hour release (his ask 2026-10-11) and install the
# cli_pw-based API helper the laptop's bedtime engine uses over SSH.
#
# Usage (from the laptop, one sudo batch on the Pi):
#   scp bootstrap/rpi5/files/pihole-* pi-lan:/tmp/ && ssh pi-lan 'bash -s' < bootstrap/rpi5/setup-pihole-lock.sh
# Expects /tmp/telegram.env (BOT_TOKEN=..., CHAT_ID=...) copied beforehand, or an existing /root/vault/telegram.env.
#
# What it does (idempotent):
#   1. /usr/local/bin/pihole-api  (FTL cli_pw auth; needs webserver.api.cli_pw = true, the default)
#   2. /usr/local/bin/pihole-unlock + systemd timer (every 5 min)
#   3. /root/vault/ (0700): telegram.env, pihole-admin (24 random chars) — only if no password is set yet
#   4. `pihole setpassword` with that password (the web UI and the API now require it)
# Runs ON the Pi.
set -euo pipefail
echo "=== Pi-hole lock (2026-10-11) ==="
sudo install -m 0755 /tmp/pihole-api /usr/local/bin/pihole-api
sudo install -m 0755 /tmp/pihole-unlock /usr/local/bin/pihole-unlock
sudo install -m 0644 /tmp/pihole-unlock.service /tmp/pihole-unlock.timer /etc/systemd/system/
sudo mkdir -p /root/vault && sudo chmod 700 /root/vault
if [ -f /tmp/telegram.env ]; then sudo install -m 0600 /tmp/telegram.env /root/vault/telegram.env; rm -f /tmp/telegram.env; fi
sudo test -f /root/vault/telegram.env || { echo "missing /root/vault/telegram.env"; exit 1; }
rm -f /tmp/pihole-api /tmp/pihole-unlock /tmp/pihole-unlock.service /tmp/pihole-unlock.timer

# group pihole may read cli_pw (FTL writes it 0640 pihole:pihole)
id -nG pi | grep -qw pihole || sudo usermod -aG pihole pi
sudo chmod 0640 /etc/pihole/cli_pw 2>/dev/null || true

if [ -z "$(sudo pihole-FTL --config webserver.api.pwhash | tr -d '[:space:]')" ]; then
    echo "[lock] generating the admin password into /root/vault/pihole-admin"
    sudo bash -c 'umask 077; tr -dc "A-Za-z0-9" </dev/urandom | head -c 24 > /root/vault/pihole-admin; echo >> /root/vault/pihole-admin'
    sudo pihole setpassword "$(sudo cat /root/vault/pihole-admin)" >/dev/null
    echo "[lock] password set (never printed)"
else
    echo "[lock] a password is already set — not changed"
fi
sudo systemctl daemon-reload
sudo systemctl enable --now pihole-unlock.timer
echo "--- verify"
curl -s -m 5 http://127.0.0.1/api/auth; echo
sg pihole -c '/usr/local/bin/pihole-api GET /api/dns/blocking'
sudo pihole-unlock status
echo "=== done: admin locked; release = 'pihole-unlock request' (6 h) ==="
