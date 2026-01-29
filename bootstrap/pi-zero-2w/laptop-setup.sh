#!/bin/bash
# Configure laptop USB network interface for Pi Zero 2W communication
#
# Usage: ./bootstrap/pi-zero-2w/laptop-setup.sh [--persist]
#
# This script:
#   1. Detects the USB ethernet interface (created by g_ether gadget)
#   2. Assigns 10.55.0.1/24 to the interface
#   3. Enables IP forwarding
#   4. Configures NAT for Pi internet access
#   5. (Optional) Installs udev rule for persistence

set -e

PERSIST=false
if [[ "$1" == "--persist" ]]; then
    PERSIST=true
fi

# === Detect USB Ethernet Interface ===
echo "=== Laptop Network Setup for Pi Zero 2W ==="
echo ""
echo "[1/4] Detecting USB ethernet interface..."

# Wait for interface to appear (Pi may still be booting)
MAX_WAIT=30
WAITED=0
USB_IFACE=""

while [[ -z "$USB_IFACE" ]] && [[ $WAITED -lt $MAX_WAIT ]]; do
    # Look for USB ethernet interfaces (common naming patterns)
    # enp0s20f0u* - Thinkpad USB
    # enp*u* - Generic USB ethernet
    # usb0 - Legacy naming
    USB_IFACE=$(ip link show 2>/dev/null | grep -oE 'enp[0-9]+s[0-9]+f[0-9]+u[0-9]+' | head -1)

    if [[ -z "$USB_IFACE" ]]; then
        USB_IFACE=$(ip link show 2>/dev/null | grep -oE 'enp[0-9]+u[0-9]+' | head -1)
    fi

    if [[ -z "$USB_IFACE" ]]; then
        USB_IFACE=$(ip link show 2>/dev/null | grep -oE 'usb[0-9]+' | head -1)
    fi

    if [[ -z "$USB_IFACE" ]]; then
        echo "  Waiting for USB ethernet interface... ($WAITED/$MAX_WAIT sec)"
        sleep 1
        ((WAITED++))
    fi
done

if [[ -z "$USB_IFACE" ]]; then
    echo "ERROR: No USB ethernet interface found"
    echo ""
    echo "Troubleshooting:"
    echo "  1. Is Pi connected via USB DATA port (inner port)?"
    echo "  2. Has Pi booted? (Wait ~45 seconds after power on)"
    echo "  3. Check with: ip link show"
    echo ""
    echo "Available interfaces:"
    ip link show | grep -E '^[0-9]+:' | cut -d: -f2
    exit 1
fi

echo "  Found: $USB_IFACE"

# === Bring Interface Up ===
echo ""
echo "[2/4] Configuring interface $USB_IFACE..."

sudo ip link set "$USB_IFACE" up
sudo ip addr flush dev "$USB_IFACE" 2>/dev/null || true
sudo ip addr add 10.55.0.1/24 dev "$USB_IFACE"

echo "  - Assigned 10.55.0.1/24 to $USB_IFACE"

# === Enable IP Forwarding ===
echo ""
echo "[3/4] Enabling IP forwarding..."

sudo sysctl -w net.ipv4.ip_forward=1 > /dev/null
echo "  - Enabled net.ipv4.ip_forward=1"

# === Configure NAT ===
echo ""
echo "[4/4] Configuring NAT for Pi internet access..."

DEFAULT_IFACE=$(ip route | grep '^default' | awk '{print $5}' | head -1)

if [[ -z "$DEFAULT_IFACE" ]]; then
    echo "  WARNING: No default route found, skipping NAT"
    echo "  Pi will not have internet access"
else
    sudo iptables -t nat -C POSTROUTING -s 10.55.0.0/24 -o "$DEFAULT_IFACE" -j MASQUERADE 2>/dev/null || \
        sudo iptables -t nat -A POSTROUTING -s 10.55.0.0/24 -o "$DEFAULT_IFACE" -j MASQUERADE

    sudo iptables -C FORWARD -s 10.55.0.0/24 -j ACCEPT 2>/dev/null || \
        sudo iptables -A FORWARD -s 10.55.0.0/24 -j ACCEPT
    sudo iptables -C FORWARD -d 10.55.0.0/24 -j ACCEPT 2>/dev/null || \
        sudo iptables -A FORWARD -d 10.55.0.0/24 -j ACCEPT

    echo "  - NAT configured: 10.55.0.0/24 -> $DEFAULT_IFACE"
fi

# === Persistence (Optional) ===
if $PERSIST; then
    echo ""
    echo "[Extra] Installing persistent configuration..."

    NM_CONN_FILE="/etc/NetworkManager/system-connections/pi-usb-gadget.nmconnection"
    sudo tee "$NM_CONN_FILE" > /dev/null << EOF
[connection]
id=Pi USB Gadget
type=ethernet
interface-name=$USB_IFACE
autoconnect=true

[ethernet]

[ipv4]
method=manual
addresses=10.55.0.1/24

[ipv6]
method=disabled
EOF
    sudo chmod 600 "$NM_CONN_FILE"
    echo "  - Created NetworkManager connection: $NM_CONN_FILE"

    SYSCTL_CONF="/etc/sysctl.d/99-pi-forward.conf"
    echo "net.ipv4.ip_forward=1" | sudo tee "$SYSCTL_CONF" > /dev/null
    echo "  - Created sysctl config: $SYSCTL_CONF"

    echo ""
    echo "NOTE: iptables NAT rules are not persisted."
    echo "For iptables persistence, install 'iptables-persistent' or use firewalld."
fi

# === Verify ===
echo ""
echo "=== Configuration Complete ==="
echo ""
echo "Interface: $USB_IFACE"
echo "Laptop IP: 10.55.0.1"
echo "Pi IP:     10.55.0.2"
echo ""

echo "Testing connectivity to Pi..."
if ping -c 1 -W 2 10.55.0.2 > /dev/null 2>&1; then
    echo "  SUCCESS: Pi is reachable at 10.55.0.2"
    echo ""
    echo "Next: ssh pi@10.55.0.2"
else
    echo "  WAITING: Pi not yet responding"
    echo "  First boot takes ~45 seconds"
    echo ""
    echo "  Try: ping 10.55.0.2"
    echo "  Then: ssh pi@10.55.0.2"
fi
echo ""
