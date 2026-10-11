#!/bin/bash
# Install the Pi-hole web UI (his ask 2026-10-11: "see what switches and knobs are ON, and also
# trace what all requests have been blocked") as /usr/local/bin/pihole-ui + pihole-ui.service on :8780.
#
# Usage (from the laptop):
#   scp bootstrap/rpi5/files/pihole-ui.py bootstrap/rpi5/files/pihole-ui.service pi-lan:/tmp/ \
#     && ssh pi-lan 'bash -s' < bootstrap/rpi5/setup-pihole-ui.sh
#
# Needs (installed by setup-pihole-lock.sh and the pause tool): /usr/local/bin/pihole-api,
# /usr/local/bin/pihole-unlock, /usr/local/bin/pihole-pause; user pi in group pihole with NOPASSWD sudo.
# The UI can only change things through pihole-pause (4 groups, <= 240 min, reason) and
# pihole-unlock (6-hour release). It never logs in to the admin and never touches global blocking
# or the Default (porn + ads) group. Idempotent. Runs ON the Pi.
set -euo pipefail
echo "=== pihole-ui (2026-10-11) ==="
for t in pihole-api pihole-unlock pihole-pause; do
    [ -x /usr/local/bin/$t ] || echo "WARN: /usr/local/bin/$t missing (the page will show an error for it)"
done
if [ -f /tmp/pihole-ui.py ]; then
    sudo install -m 0755 /tmp/pihole-ui.py /usr/local/bin/pihole-ui
    rm -f /tmp/pihole-ui.py
fi
if [ -f /tmp/pihole-ui.service ]; then
    sudo install -m 0644 /tmp/pihole-ui.service /etc/systemd/system/pihole-ui.service
    rm -f /tmp/pihole-ui.service
fi
[ -x /usr/local/bin/pihole-ui ] || { echo "missing /usr/local/bin/pihole-ui (scp the files to /tmp first)"; exit 1; }
id -nG pi | grep -qw pihole || sudo usermod -aG pihole pi
python3 -c "import ast; ast.parse(open('/usr/local/bin/pihole-ui').read())"
sudo systemctl daemon-reload
sudo systemctl enable pihole-ui.service >/dev/null
sudo systemctl restart pihole-ui.service
for _ in $(seq 1 20); do curl -fsS -m 2 http://127.0.0.1:8780/healthz >/dev/null 2>&1 && break; sleep 0.5; done
echo "--- verify"
systemctl is-active pihole-ui.service
curl -s -m 20 http://127.0.0.1:8780/api/state | head -c 400; echo
ip=$(hostname -I | tr ' ' '\n' | grep '^192\.168\.0\.' | head -n1)
echo "=== done: open http://${ip:-192.168.0.2}:8780/  (check: curl -s http://${ip:-192.168.0.2}:8780/api/state) ==="
