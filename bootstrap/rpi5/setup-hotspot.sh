#!/bin/bash
# Set up WiFi hotspot on Raspberry Pi 5 using hostapd + dnsmasq
#
# Usage: ssh pi@<IP> 'bash -s' < bootstrap/rpi5/setup-hotspot.sh
#   Or with secrets: ssh pi@<IP> 'WIFI_SSID=MyNet WIFI_PASSPHRASE=secret bash -s' < bootstrap/rpi5/setup-hotspot.sh
#
# Secrets can be provided via:
#   1. Environment variables: WIFI_SSID, WIFI_PASSPHRASE
#   2. Interactive prompt (if not set)
#
# Runs ON the Pi (not on host).

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

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
echo "[1/4] Installing hostapd and dnsmasq..."
sudo apt install -y hostapd dnsmasq

# === Configure hostapd ===
echo "[2/4] Configuring hostapd..."

# If running via ssh pipe, templates won't be at SCRIPT_DIR
# Check if template exists locally, otherwise use inline
if [[ -f "$SCRIPT_DIR/hostapd.conf.template" ]]; then
    TEMPLATE="$SCRIPT_DIR/hostapd.conf.template"
    sudo cp "$TEMPLATE" /etc/hostapd/hostapd.conf
else
    # Inline template (for ssh pipe usage)
    sudo tee /etc/hostapd/hostapd.conf > /dev/null << 'TMPL'
interface=wlan0_ap
driver=nl80211
ssid=%%WIFI_SSID%%
hw_mode=g
channel=6
wmm_enabled=0
macaddr_acl=0
auth_algs=1
wpa=2
wpa_passphrase=%%WIFI_PASSPHRASE%%
wpa_key_mgmt=WPA-PSK
rsn_pairwise=CCMP
country_code=IN
ieee80211n=1
TMPL
fi

# Substitute placeholders
sudo sed -i "s/%%WIFI_SSID%%/$WIFI_SSID/" /etc/hostapd/hostapd.conf
sudo sed -i "s/%%WIFI_PASSPHRASE%%/$WIFI_PASSPHRASE/" /etc/hostapd/hostapd.conf

# Point hostapd to config
sudo sed -i 's|^#DAEMON_CONF=""$|DAEMON_CONF="/etc/hostapd/hostapd.conf"|' /etc/default/hostapd 2>/dev/null || true

echo "  - Installed hostapd config with SSID: $WIFI_SSID"

# === Configure dnsmasq ===
echo "[3/4] Configuring dnsmasq..."

if [[ -f "$SCRIPT_DIR/dnsmasq.conf" ]]; then
    sudo cp "$SCRIPT_DIR/dnsmasq.conf" /etc/dnsmasq.d/hotspot.conf
else
    sudo tee /etc/dnsmasq.d/hotspot.conf > /dev/null << 'EOF'
interface=wlan0_ap
dhcp-range=192.168.4.2,192.168.4.20,255.255.255.0,24h
address=/gw.wlan/192.168.4.1
EOF
fi

echo "  - Installed dnsmasq config (DHCP: 192.168.4.2-20)"

# === Enable and start services ===
echo "[4/4] Enabling services..."

sudo systemctl unmask hostapd
sudo systemctl enable hostapd dnsmasq
sudo systemctl restart dnsmasq
sudo systemctl restart hostapd

echo ""
echo "=== Hotspot Setup Complete ==="
echo ""
echo "SSID: $WIFI_SSID"
echo "AP Interface: wlan0_ap"
echo "DHCP Range: 192.168.4.2 - 192.168.4.20"
echo ""
echo "Commands:"
echo "  sudo systemctl status hostapd     # AP status"
echo "  sudo systemctl status dnsmasq     # DHCP status"
echo "  iw dev                            # Wireless interfaces"
