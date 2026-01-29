#!/bin/bash
# Set up NAT and IP forwarding for hotspot clients
#
# Routes traffic from wlan0 (hotspot) to eth0 (internet uplink).
# Persisted via netfilter-persistent.
#
# Usage: ssh pi@<IP> 'bash -s' < bootstrap/rpi5/setup-nat.sh
#
# Runs ON the Pi (not on host).

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

# Forward: allow hotspot clients to reach internet via eth0
sudo iptables -A FORWARD -i wlan0 -o eth0 -j ACCEPT
sudo iptables -A FORWARD -i eth0 -o wlan0 -m state --state RELATED,ESTABLISHED -j ACCEPT

# Save rules
sudo netfilter-persistent save

echo ""
echo "=== NAT Setup Complete ==="
echo "  - IP forwarding: enabled"
echo "  - NAT: wlan0 -> eth0 (MASQUERADE)"
echo "  - Forward: wlan0 <-> eth0 (stateful)"
echo "  - Rules persisted via netfilter-persistent"
