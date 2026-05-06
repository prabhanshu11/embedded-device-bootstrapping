#!/bin/bash
# ⚠️  OBSOLETE — DO NOT RUN ON FRESH FLASH ⚠️
#
# This script describes the pre-2026-04-10 architecture where the Pi5 acted as
# its own WiFi access point (hostapd on wlan0_ap, 192.168.50.0/24). On 2026-04-10
# we pivoted: Archer C6 became the primary AP, Pi5 demoted to DNS-only at
# 192.168.0.2 on the Archer LAN. Running this script today would re-enable
# hostapd and conflict with the Archer C6 setup.
#
# Kept for historical reference only. To re-enable Pi5-as-hotspot you must
# also reverse the changes documented in:
#   ~/.claude/projects/-home-prabhanshu-Programs/memory/feedback_pi5_dns_changes.md
#   ~/.claude/projects/-home-prabhanshu-Programs/memory/reference_pi5_ssh.md
#
# --- ORIGINAL HEADER (pre-Apr-10) ---
# Set up WiFi hotspot on Raspberry Pi 5 using hostapd + Pi-hole FTL (DHCP)
# hostapd runs on wlan0_ap (virtual AP interface). Pi-hole FTL handles DHCP.
# Usage: ssh pi@<IP> 'bash -s' < bootstrap/rpi5/setup-hotspot.sh

echo "ERROR: setup-hotspot.sh is OBSOLETE (pre-2026-04-10 architecture)."
echo "  Pi5 is no longer a hotspot — Archer C6 is the AP. Pi5 = DNS-only."
echo "  See header comment for context. Refusing to run."
echo "  If you really mean to revert architecture, edit this script to remove this guard."
exit 2

# (legacy body retained below for historical reference, but unreachable)

set -e

echo "=== RPi5 WiFi Hotspot Setup ==="
echo ""

# === Get secrets ===
if [[ -z "$WIFI_SSID" ]]; then
    read -p "WiFi SSID for hotspot: " WIFI_SSID
fi

if [[ -z "$WIFI_PASSPHRASE" ]]; then
    read -sp "WiFi passphrase: " WIFI_PASSPHRASE
    echo
fi

if [[ ${#WIFI_PASSPHRASE} -lt 8 ]]; then
    echo "ERROR: WPA passphrase must be at least 8 characters"
    exit 1
fi

# === Install packages ===
echo "[1/5] Installing hostapd..."
sudo apt install -y hostapd

# === Stop wpa_supplicant (conflicts with hostapd on wlan0) ===
echo "[2/5] Disabling wpa_supplicant@wlan0..."
sudo systemctl stop wpa_supplicant@wlan0 2>/dev/null || true
sudo systemctl disable wpa_supplicant@wlan0 2>/dev/null || true

# === Configure hostapd ===
echo "[3/5] Configuring hostapd..."

sudo tee /etc/hostapd/hostapd.conf > /dev/null << TMPL
interface=wlan0_ap
driver=nl80211
ssid=${WIFI_SSID}
hw_mode=g
channel=6
wmm_enabled=1
macaddr_acl=0
auth_algs=1
wpa=2
wpa_passphrase=${WIFI_PASSPHRASE}
wpa_key_mgmt=WPA-PSK
rsn_pairwise=CCMP
country_code=IN
ieee80211n=1
logger_syslog_level=0
ctrl_interface=/var/run/hostapd
TMPL

echo "  - Installed hostapd config with SSID: $WIFI_SSID"

# === Create virtual AP interface and assign IP ===
echo "[4/5] Creating wlan0_ap and assigning hotspot IP..."
sudo iw dev wlan0 interface add wlan0_ap type __ap 2>/dev/null || true
sudo ip addr add 192.168.50.1/24 dev wlan0_ap 2>/dev/null || true
sudo ip link set wlan0_ap up

# === Enable and start hostapd ===
echo "[5/5] Enabling hostapd..."
sudo systemctl unmask hostapd
sudo systemctl enable hostapd
sudo systemctl restart hostapd

echo ""
echo "=== Hotspot Setup Complete ==="
echo ""
echo "SSID: $WIFI_SSID"
echo "AP Interface: wlan0_ap"
echo "Gateway: 192.168.50.1"
echo ""
echo "NOTE: DHCP is handled by Pi-hole FTL, not dnsmasq."
echo "      Run setup-pihole.sh next if Pi-hole is not installed."
echo ""
echo "Commands:"
echo "  sudo systemctl status hostapd     # AP status"
echo "  journalctl -u hostapd -f          # AP logs (WPA handshake)"
echo "  iw dev                            # Wireless interfaces"
