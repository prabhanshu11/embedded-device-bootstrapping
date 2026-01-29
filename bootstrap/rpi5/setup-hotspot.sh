#!/bin/bash
# Set up WiFi hotspot on Raspberry Pi 5 using hostapd + Pi-hole FTL (DHCP)
#
# Architecture: hostapd runs directly on wlan0 (not virtual wlan0_ap).
# brcmfmac firmware does not deliver EAPOL frames through virtual AP interfaces.
# Trade-off: no WiFi client fallback — Pi relies on eth0 for uplink.
#
# Usage: ssh pi@<IP> 'bash -s' < bootstrap/rpi5/setup-hotspot.sh
#   Or: ssh pi@<IP> 'WIFI_SSID=MyNet WIFI_PASSPHRASE=secret bash -s' < bootstrap/rpi5/setup-hotspot.sh
#
# Runs ON the Pi (not on host).

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
interface=wlan0
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

# === Assign AP IP to wlan0 ===
echo "[4/5] Assigning hotspot IP..."
sudo ip addr add 192.168.50.1/24 dev wlan0 2>/dev/null || true
sudo ip link set wlan0 up

# === Enable and start hostapd ===
echo "[5/5] Enabling hostapd..."
sudo systemctl unmask hostapd
sudo systemctl enable hostapd
sudo systemctl restart hostapd

echo ""
echo "=== Hotspot Setup Complete ==="
echo ""
echo "SSID: $WIFI_SSID"
echo "AP Interface: wlan0"
echo "Gateway: 192.168.50.1"
echo ""
echo "NOTE: DHCP is handled by Pi-hole FTL, not dnsmasq."
echo "      Run setup-pihole.sh next if Pi-hole is not installed."
echo ""
echo "Commands:"
echo "  sudo systemctl status hostapd     # AP status"
echo "  journalctl -u hostapd -f          # AP logs (WPA handshake)"
echo "  iw dev                            # Wireless interfaces"
