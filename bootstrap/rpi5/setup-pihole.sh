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

# === Configure upstream DNS ===
# ============================================================================
# ⚠️  DNS LANDMINE — DO NOT CHANGE WITHOUT READING THIS
# ============================================================================
# Use CleanBrowsing ADULT Filter (.10/.11) ONLY. NEVER use Family Filter (.168).
#
# CleanBrowsing Family Filter (185.228.168.168 / 185.228.169.168) forces
# YouTube into Restricted Mode at the DNS level — Google's edge servers detect
# the DNS path and silently restrict content. The user CANNOT toggle this off.
# It will break legitimate YouTube viewing (Sam Harris podcasts, political
# commentary, anything age-flagged).
#
# This trap has been triggered TWICE already:
#   - 2026-04-01: Earlier session set Family Filter as "extra protection"
#   - 2026-04-09: Archer C6 migration set .168 as DHCP secondary DNS
# Both times the user lost YouTube and had to debug from scratch.
#
# Adult Filter (185.228.168.10 / 185.228.169.11) gives the same porn blocking
# WITHOUT poisoning YouTube. This is the only acceptable upstream.
#
# If you ever feel tempted to "add Family for more protection" — DON'T.
# Read pi-noc/CLAUDE.md "DNS Landmine" section.
# ============================================================================
echo "[2/3] Setting upstream DNS (CleanBrowsing Adult Filter)..."
sudo pihole-FTL --config dns.upstreams '["185.228.168.10", "185.228.169.11"]'

# === Configure DHCP for hotspot ===
echo "[3/3] Configuring DHCP for hotspot network..."

sudo pihole-FTL --config dns.interface wlan0_ap
sudo pihole-FTL --config dhcp.start 192.168.50.10
sudo pihole-FTL --config dhcp.end 192.168.50.250
sudo pihole-FTL --config dhcp.router 192.168.50.1
sudo pihole-FTL --config dhcp.netmask 255.255.255.0
sudo pihole-FTL --config dhcp.leaseTime 24h
sudo pihole-FTL --config dhcp.active true

sudo systemctl restart pihole-FTL

echo ""
echo "=== Pi-hole Setup Complete ==="
echo "  - Upstream DNS: CleanBrowsing Adult Filter (185.228.168.10)"
echo "  - DNS: listening on wlan0_ap (192.168.50.1:53)"
echo "  - DHCP: 192.168.50.10 - 192.168.50.250"
echo "  - Gateway: 192.168.50.1"
echo "  - Web UI: http://192.168.50.1/admin"
