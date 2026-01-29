#!/bin/bash
# Flash and configure SD card for Raspberry Pi devices
#
# Usage: ./flash/flash-sd.sh --device /dev/mmcblk0 --profile pi-zero-2w [options]
#
# Profiles:
#   pi-zero-2w  - USB gadget (dwc2, g_ether), static IP 10.55.0.2
#   rpi5        - Standard ethernet, optional static IP
#   generic     - Just hostname, SSH, user (no gadget or static IP)
#
# Options:
#   --device DEV        Block device (required)
#   --profile PROFILE   Device profile: pi-zero-2w, rpi5, generic (required)
#   --hostname NAME     Pi hostname (default: profile-based)
#   --password PASS     Password for 'pi' user (default: raspberry)
#   --wifi SSID PASS    Configure WiFi via NetworkManager
#   --static-ip IP/CIDR Static IP for eth0 (rpi5 profile only)
#   --force             Skip confirmation prompt
#
# Downloads Raspberry Pi OS Lite (Bookworm arm64), flashes to SD card,
# and configures based on the selected profile.

set -e

# === Configuration ===
PI_OS_URL="https://downloads.raspberrypi.com/raspios_lite_arm64/images/raspios_lite_arm64-2024-11-19/2024-11-19-raspios-bookworm-arm64-lite.img.xz"
PI_OS_CHECKSUM="ab99e6a41bdc10d8bbff7edbc86d7abfe0eea2bacc109b62c7ee6f8730b5c3e8"
CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/pi-images"
IMAGE_NAME="raspios-bookworm-arm64-lite.img"

# === Defaults ===
DEVICE=""
PROFILE=""
HOSTNAME=""
PASSWORD="raspberry"
WIFI_SSID=""
WIFI_PASS=""
STATIC_IP=""
FORCE=false

# === Parse Arguments ===
while [[ $# -gt 0 ]]; do
    case "$1" in
        --device)
            DEVICE="$2"
            shift 2
            ;;
        --profile)
            PROFILE="$2"
            shift 2
            ;;
        --hostname)
            HOSTNAME="$2"
            shift 2
            ;;
        --password)
            PASSWORD="$2"
            shift 2
            ;;
        --wifi)
            WIFI_SSID="$2"
            WIFI_PASS="$3"
            shift 3
            ;;
        --static-ip)
            STATIC_IP="$2"
            shift 2
            ;;
        --force)
            FORCE=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# === Profile Defaults ===
case "$PROFILE" in
    pi-zero-2w)
        HOSTNAME="${HOSTNAME:-pi-keyboard}"
        ;;
    rpi5)
        HOSTNAME="${HOSTNAME:-pi-hub}"
        ;;
    generic)
        HOSTNAME="${HOSTNAME:-raspberrypi}"
        ;;
    *)
        echo "Usage: $0 --device /dev/mmcblk0 --profile <pi-zero-2w|rpi5|generic> [options]"
        echo ""
        echo "Options:"
        echo "  --device DEV        Block device (required)"
        echo "  --profile PROFILE   Device profile (required)"
        echo "  --hostname NAME     Pi hostname (default: profile-based)"
        echo "  --password PASS     Password for 'pi' user (default: raspberry)"
        echo "  --wifi SSID PASS    Configure WiFi via NetworkManager"
        echo "  --static-ip IP/CIDR Static IP for eth0 (rpi5 only)"
        echo "  --force             Skip confirmation prompt"
        exit 1
        ;;
esac

# === Validation ===
if [[ -z "$DEVICE" ]]; then
    echo "ERROR: --device is required"
    echo ""
    echo "Available block devices:"
    lsblk -d -o NAME,SIZE,MODEL | grep -v "^loop"
    exit 1
fi

if [[ ! -b "$DEVICE" ]]; then
    echo "ERROR: $DEVICE is not a block device"
    echo ""
    echo "Available block devices:"
    lsblk -d -o NAME,SIZE,MODEL | grep -v "^loop"
    exit 1
fi

# Safety check - don't flash to main drive
if [[ "$DEVICE" == "/dev/sda" ]] || [[ "$DEVICE" == "/dev/nvme0n1" ]]; then
    echo "ERROR: Refusing to flash to $DEVICE (looks like a main drive)"
    exit 1
fi

# Check for required tools
for cmd in wget sha256sum xz dd mount umount openssl; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "ERROR: Required command '$cmd' not found"
        exit 1
    fi
done

echo "=== Raspberry Pi SD Card Flasher ==="
echo ""
echo "Device:   $DEVICE"
echo "Profile:  $PROFILE"
echo "Hostname: $HOSTNAME"
echo "Password: $PASSWORD"
[[ -n "$WIFI_SSID" ]] && echo "WiFi:     $WIFI_SSID"
[[ -n "$STATIC_IP" ]] && echo "Static IP: $STATIC_IP"
echo ""

# Confirm before proceeding
if [[ "$FORCE" != "true" ]] && [[ -t 0 ]]; then
    read -p "This will ERASE ALL DATA on $DEVICE. Continue? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Aborted."
        exit 1
    fi
else
    echo "Non-interactive mode - proceeding with flash..."
fi

# === Download/Cache Image ===
mkdir -p "$CACHE_DIR"
COMPRESSED_IMAGE="$CACHE_DIR/${IMAGE_NAME}.xz"
DECOMPRESSED_IMAGE="$CACHE_DIR/$IMAGE_NAME"

if [[ ! -f "$DECOMPRESSED_IMAGE" ]]; then
    echo ""
    echo "[1/5] Downloading Raspberry Pi OS..."

    if [[ ! -f "$COMPRESSED_IMAGE" ]]; then
        wget -c "$PI_OS_URL" -O "$COMPRESSED_IMAGE.tmp"
        mv "$COMPRESSED_IMAGE.tmp" "$COMPRESSED_IMAGE"
    fi

    echo "[1/5] Verifying checksum..."
    ACTUAL_CHECKSUM=$(sha256sum "$COMPRESSED_IMAGE" | cut -d' ' -f1)
    if [[ "$ACTUAL_CHECKSUM" != "$PI_OS_CHECKSUM" ]]; then
        echo "ERROR: Checksum mismatch!"
        echo "Expected: $PI_OS_CHECKSUM"
        echo "Got:      $ACTUAL_CHECKSUM"
        rm -f "$COMPRESSED_IMAGE"
        exit 1
    fi
    echo "Checksum OK"

    echo "[1/5] Decompressing image..."
    xz -dk "$COMPRESSED_IMAGE"
else
    echo "[1/5] Using cached image: $DECOMPRESSED_IMAGE"
fi

# === Flash Image ===
echo ""
echo "[2/5] Flashing image to $DEVICE..."

# Unmount any mounted partitions
for part in ${DEVICE}p* ${DEVICE}[0-9]*; do
    if mountpoint -q "$part" 2>/dev/null || mount | grep -q "^$part "; then
        echo "Unmounting $part..."
        umount "$part" 2>/dev/null || true
    fi
done

# Flash with progress
dd if="$DECOMPRESSED_IMAGE" of="$DEVICE" bs=4M status=progress conv=fsync

echo "Syncing..."
sync

# Re-read partition table
partprobe "$DEVICE" 2>/dev/null || true
sleep 2

# === Determine Partition Names ===
if [[ "$DEVICE" == *"mmcblk"* ]] || [[ "$DEVICE" == *"loop"* ]]; then
    BOOT_PART="${DEVICE}p1"
    ROOT_PART="${DEVICE}p2"
else
    BOOT_PART="${DEVICE}1"
    ROOT_PART="${DEVICE}2"
fi

echo "Boot partition: $BOOT_PART"
echo "Root partition: $ROOT_PART"

# Wait for partitions to appear
for i in {1..10}; do
    if [[ -b "$BOOT_PART" ]] && [[ -b "$ROOT_PART" ]]; then
        break
    fi
    echo "Waiting for partitions..."
    sleep 1
done

if [[ ! -b "$BOOT_PART" ]] || [[ ! -b "$ROOT_PART" ]]; then
    echo "ERROR: Partitions not found after flashing"
    exit 1
fi

# === Configure Boot Partition ===
echo ""
echo "[3/5] Configuring boot partition..."

BOOT_MNT=$(mktemp -d)
mount "$BOOT_PART" "$BOOT_MNT"

# Enable SSH (empty file)
touch "$BOOT_MNT/ssh"
echo "  - Created ssh file (enables SSH on first boot)"

# Create user with password (Bookworm has no default user)
PASSWORD_HASH=$(echo "$PASSWORD" | openssl passwd -6 -stdin)
echo "pi:$PASSWORD_HASH" > "$BOOT_MNT/userconf"
echo "  - Created userconf (user: pi, password: $PASSWORD)"

# Profile-specific boot config
if [[ "$PROFILE" == "pi-zero-2w" ]]; then
    # Enable USB gadget in config.txt
    if ! grep -q "dtoverlay=dwc2" "$BOOT_MNT/config.txt"; then
        echo "dtoverlay=dwc2" >> "$BOOT_MNT/config.txt"
        echo "  - Added dtoverlay=dwc2 to config.txt"
    fi

    # Add USB gadget module to cmdline.txt (must stay single line!)
    if ! grep -q "modules-load=dwc2,g_ether" "$BOOT_MNT/cmdline.txt"; then
        sed -i 's/rootwait/rootwait modules-load=dwc2,g_ether/' "$BOOT_MNT/cmdline.txt"
        echo "  - Added modules-load=dwc2,g_ether to cmdline.txt"
    fi
fi

umount "$BOOT_MNT"
rmdir "$BOOT_MNT"

# === Configure Root Partition ===
echo ""
echo "[4/5] Configuring root partition..."

ROOT_MNT=$(mktemp -d)
mount "$ROOT_PART" "$ROOT_MNT"

# Set hostname
echo "$HOSTNAME" > "$ROOT_MNT/etc/hostname"
sed -i "s/127.0.1.1.*/127.0.1.1\t$HOSTNAME/" "$ROOT_MNT/etc/hosts"
echo "  - Set hostname to $HOSTNAME"

NM_CONN_DIR="$ROOT_MNT/etc/NetworkManager/system-connections"
mkdir -p "$NM_CONN_DIR"

# Profile-specific network config
if [[ "$PROFILE" == "pi-zero-2w" ]]; then
    # USB gadget network (static IP 10.55.0.2)
    cat > "$NM_CONN_DIR/usb0.nmconnection" << 'EOF'
[connection]
id=USB Gadget Network
type=ethernet
interface-name=usb0
autoconnect=true

[ethernet]

[ipv4]
method=manual
addresses=10.55.0.2/24
gateway=10.55.0.1
dns=8.8.8.8;1.1.1.1;

[ipv6]
method=disabled
EOF
    chmod 600 "$NM_CONN_DIR/usb0.nmconnection"
    echo "  - Created USB gadget NM config (IP: 10.55.0.2)"
fi

if [[ "$PROFILE" == "rpi5" ]] && [[ -n "$STATIC_IP" ]]; then
    # Static ethernet IP for RPi5
    cat > "$NM_CONN_DIR/eth0-static.nmconnection" << EOF
[connection]
id=Wired Static
type=ethernet
interface-name=eth0
autoconnect=true

[ethernet]

[ipv4]
method=manual
addresses=$STATIC_IP
dns=8.8.8.8;1.1.1.1;

[ipv6]
method=disabled
EOF
    chmod 600 "$NM_CONN_DIR/eth0-static.nmconnection"
    echo "  - Created static ethernet NM config (IP: $STATIC_IP)"
fi

# WiFi configuration (any profile)
if [[ -n "$WIFI_SSID" ]]; then
    cat > "$NM_CONN_DIR/wifi.nmconnection" << EOF
[connection]
id=$WIFI_SSID
type=wifi
autoconnect=true

[wifi]
mode=infrastructure
ssid=$WIFI_SSID

[wifi-security]
key-mgmt=wpa-psk
psk=$WIFI_PASS

[ipv4]
method=auto

[ipv6]
method=disabled
EOF
    chmod 600 "$NM_CONN_DIR/wifi.nmconnection"
    echo "  - Created WiFi NM config (SSID: $WIFI_SSID)"
fi

umount "$ROOT_MNT"
rmdir "$ROOT_MNT"

# === Done ===
echo ""
echo "[5/5] Syncing..."
sync

echo ""
echo "=== SD Card Ready ==="
echo ""
echo "Profile: $PROFILE"
echo "Hostname: $HOSTNAME"
echo "User: pi / $PASSWORD"
[[ -n "$WIFI_SSID" ]] && echo "WiFi: $WIFI_SSID"
echo ""

case "$PROFILE" in
    pi-zero-2w)
        echo "Next steps:"
        echo "  1. Insert SD card into Pi Zero 2W"
        echo "  2. Connect Pi USB DATA port (inner port) to laptop"
        echo "  3. Wait ~45 seconds for Pi to boot"
        echo "  4. Run: ./bootstrap/pi-zero-2w/laptop-setup.sh"
        echo ""
        echo "Network:"
        echo "  Pi IP:     10.55.0.2"
        echo "  Laptop IP: 10.55.0.1"
        ;;
    rpi5)
        echo "Next steps:"
        echo "  1. Insert SD card into Raspberry Pi 5"
        echo "  2. Connect ethernet and power"
        echo "  3. Wait ~60 seconds for first boot"
        if [[ -n "$STATIC_IP" ]]; then
            echo "  4. SSH: ssh pi@${STATIC_IP%%/*}"
        elif [[ -n "$WIFI_SSID" ]]; then
            echo "  4. SSH: ssh pi@${HOSTNAME}.local"
        else
            echo "  4. Find Pi IP and SSH in"
        fi
        ;;
    generic)
        echo "Next steps:"
        echo "  1. Insert SD card into Raspberry Pi"
        echo "  2. Boot and find Pi on network"
        echo "  3. SSH: ssh pi@${HOSTNAME}.local"
        ;;
esac
echo ""
