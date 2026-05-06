#!/bin/bash
# Install and enable Tailscale on RPi5 so it's reachable by hostname (`rpi5`)
# from any device on the tailnet, regardless of LAN IP.
#
# This is the canonical way to reach the Pi after 2026-04-10. LAN IPs are
# fallback only — see ~/.claude/projects/-home-prabhanshu-Programs/memory/reference_pi5_ssh.md
#
# Usage: ssh pi@<IP> 'bash -s' < bootstrap/rpi5/setup-tailscale.sh
#
# After this script finishes, run on the Pi:
#     sudo tailscale up --hostname=rpi5 --ssh
# and click the printed auth URL in a browser logged into the same tailnet.

set -e

echo "=== RPi5 Tailscale Setup ==="
echo ""

if command -v tailscale &>/dev/null; then
    echo "[1/2] Tailscale already installed: $(tailscale version | head -1)"
else
    echo "[1/2] Installing Tailscale..."
    curl -fsSL https://tailscale.com/install.sh | sh
fi

echo "[2/2] Enabling tailscaled..."
sudo systemctl enable --now tailscaled

echo ""
echo "Done. To bring the Pi online on your tailnet, run:"
echo "    sudo tailscale up --hostname=rpi5 --ssh"
echo ""
echo "Then verify from any other tailnet device:"
echo "    tailscale status | grep rpi5"
echo "    ssh pi@rpi5"
