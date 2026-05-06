#!/bin/bash
# ⚠️  OBSOLETE — DO NOT RUN ON FRESH FLASH ⚠️
#
# Pre-2026-04-10 NAT/forwarding for Pi5-as-hotspot. Pi5 is no longer a hotspot
# (Archer C6 took over on 2026-04-10). Adding iptables rules here would also
# violate the explicit constraint in feedback_pi5_dns_changes.md ("no iptables
# tricks on the Pi5"). Kept for historical reference only.
#
# --- ORIGINAL HEADER ---
# Set up NAT and IP forwarding for hotspot clients
# Routes traffic from wlan0_ap (hotspot) to eth0 (internet uplink).
# Persisted via netfilter-persistent.

echo "ERROR: setup-nat.sh is OBSOLETE (pre-2026-04-10 architecture)."
echo "  Pi5 no longer routes traffic — see embedded-device-bootstrapping/CLAUDE.md."
echo "  Refusing to run."
exit 2

set -e

echo "=== RPi5 NAT + Forwarding Setup ==="
echo ""

# === Install netfilter-persistent ===
echo "[1/3] Installing iptables-persistent..."
sudo DEBIAN_FRONTEND=noninteractive apt install -y iptables-persistent

# === Enable IP forwarding ===
echo "[2/3] Enabling IP forwarding..."
sudo sysctl -w net.ipv4.ip_forward=1
if ! grep -q '^net.ipv4.ip_forward=1' /etc/sysctl.conf; then
    echo 'net.ipv4.ip_forward=1' | sudo tee -a /etc/sysctl.conf
fi

# === Set iptables rules ===
echo "[3/3] Configuring iptables rules..."

# Flush existing rules
sudo iptables -t nat -F
sudo iptables -F FORWARD

# NAT: masquerade outgoing traffic on eth0
sudo iptables -t nat -A POSTROUTING -o eth0 -j MASQUERADE

# Forward: allow hotspot clients (wlan0_ap) to reach internet via eth0
sudo iptables -A FORWARD -i wlan0_ap -o eth0 -j ACCEPT
sudo iptables -A FORWARD -i eth0 -o wlan0_ap -m state --state RELATED,ESTABLISHED -j ACCEPT

# Save rules
sudo netfilter-persistent save

echo ""
echo "=== NAT Setup Complete ==="
echo "  - IP forwarding: enabled"
echo "  - NAT: wlan0_ap -> eth0 (MASQUERADE)"
echo "  - Forward: wlan0_ap <-> eth0 (stateful)"
echo "  - Rules persisted via netfilter-persistent"
