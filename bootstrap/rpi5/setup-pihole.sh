#!/bin/bash
# Set up Pi-hole for DNS ad-blocking + DHCP on hotspot network
#
# Pi-hole FTL handles both DNS (port 53) and DHCP (port 67) for hotspot clients.
# This replaces the need for standalone dnsmasq.
#
# Usage: ssh pi@<IP> 'bash -s' < bootstrap/rpi5/setup-pihole.sh
#
# NOTE: Pi-hole installer is interactive. This script runs it then applies
# post-install config for the hotspot DHCP setup.
#
# Runs ON the Pi (not on host).

set -e

echo "=== RPi5 Pi-hole Setup ==="
echo ""

# === Install Pi-hole ===
echo "[1/2] Installing Pi-hole..."
if command -v pihole &>/dev/null; then
    echo "  - Pi-hole already installed"
    pihole version
else
    echo "  - Running Pi-hole installer (interactive)..."
    curl -sSL https://install.pi-hole.net | bash
fi

# === Configure DHCP for hotspot ===
echo "[2/2] Configuring DHCP for hotspot network..."

sudo pihole-FTL --config dns.interface wlan0
sudo pihole-FTL --config dhcp.start 192.168.50.10
sudo pihole-FTL --config dhcp.end 192.168.50.250
sudo pihole-FTL --config dhcp.router 192.168.50.1
sudo pihole-FTL --config dhcp.netmask 255.255.255.0
sudo pihole-FTL --config dhcp.leaseTime 24h
sudo pihole-FTL --config dhcp.active true

sudo systemctl restart pihole-FTL

echo ""
echo "=== Pi-hole Setup Complete ==="
echo "  - DNS: listening on wlan0 (192.168.50.1:53)"
echo "  - DHCP: 192.168.50.10 - 192.168.50.250"
echo "  - Gateway: 192.168.50.1"
echo "  - Web UI: http://192.168.50.1/admin"
