#!/bin/bash
# Install uplink monitor service
#
# Auto-switches default route between eth0 (preferred) and wlan0 (fallback).
# Checks every 10 seconds.
#
# Usage: ssh pi@<IP> 'bash -s' < bootstrap/rpi5/setup-uplink-monitor.sh
#
# Runs ON the Pi (not on host).

set -e

echo "=== RPi5 Uplink Monitor Setup ==="
echo ""

# === Install script ===
echo "[1/2] Installing uplink-monitor.sh..."
sudo tee /usr/local/bin/uplink-monitor.sh > /dev/null << 'SCRIPT'
#!/bin/bash
# Uplink Monitor - Auto-detect and switch between Ethernet and WiFi uplink
# Hotspot stays on regardless of uplink mode

LOG_FILE=/var/log/uplink-mode.log
CHECK_INTERVAL=10
CURRENT_MODE=""

log() {
    echo "$(date '+%Y-%m-%d %H:%M:%S') - $1" >> "$LOG_FILE"
    echo "$1"
}

eth0_has_carrier() {
    [ "$(cat /sys/class/net/eth0/carrier 2>/dev/null)" = "1" ]
}

eth0_has_ip() {
    ip addr show eth0 2>/dev/null | grep -q 'inet '
}

wlan0_has_ip() {
    ip addr show wlan0 2>/dev/null | grep -q 'inet '
}

switch_to_ethernet() {
    if [ "$CURRENT_MODE" != "ETHERNET" ]; then
        log "Switching to ETHERNET uplink"
        ip route del default via $(ip route | grep 'default.*wlan0' | awk '{print $3}') 2>/dev/null
        GATEWAY=$(ip route | grep 'eth0' | grep -v default | head -1 | awk '{print $1}' | sed 's|/.*||' | awk -F. '{print $1"."$2"."$3".1"}')
        if [ -n "$GATEWAY" ]; then
            ip route add default via "$GATEWAY" dev eth0 metric 100 2>/dev/null || true
        fi
        CURRENT_MODE="ETHERNET"
        log "Now using ETHERNET uplink"
    fi
}

switch_to_wifi() {
    if [ "$CURRENT_MODE" != "WIFI" ]; then
        log "Switching to WIFI uplink"
        ip route del default via $(ip route | grep 'default.*eth0' | awk '{print $3}') 2>/dev/null
        CURRENT_MODE="WIFI"
        log "Now using WIFI uplink"
    fi
}

log "Uplink monitor started"

while true; do
    if eth0_has_carrier && eth0_has_ip; then
        switch_to_ethernet
    elif wlan0_has_ip; then
        switch_to_wifi
    else
        log "WARNING: No uplink available!"
    fi
    sleep $CHECK_INTERVAL
done
SCRIPT

sudo chmod +x /usr/local/bin/uplink-monitor.sh

# === Install systemd service ===
echo "[2/2] Installing systemd service..."
sudo tee /etc/systemd/system/uplink-monitor.service > /dev/null << 'SERVICE'
[Unit]
Description=Network Uplink Monitor (Ethernet/WiFi auto-switch)
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=/usr/local/bin/uplink-monitor.sh
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
SERVICE

sudo systemctl daemon-reload
sudo systemctl enable uplink-monitor
sudo systemctl restart uplink-monitor

echo ""
echo "=== Uplink Monitor Setup Complete ==="
echo "  - Script: /usr/local/bin/uplink-monitor.sh"
echo "  - Log: /var/log/uplink-mode.log"
echo "  - Prefers eth0, falls back to wlan0"
