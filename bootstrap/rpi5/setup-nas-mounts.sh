#!/bin/bash
# Set up NAS USB drive mount points
#
# Mounts:
#   - Samsung T7 SSD (exfat) -> /mnt/nas/t7
#   - WD Elements HDD (ntfs-3g) -> /mnt/nas/elements
#   - Android USB drive (exfat) -> /mnt/nas/android (UUID varies)
#
# Usage: ssh pi@<IP> 'bash -s' < bootstrap/rpi5/setup-nas-mounts.sh
#   Or: ssh pi@<IP> 'ANDROID_UUID=XXXX-XXXX bash -s' < bootstrap/rpi5/setup-nas-mounts.sh
#
# Runs ON the Pi (not on host).

set -e

echo "=== RPi5 NAS Mount Setup ==="
echo ""

# Known UUIDs
T7_UUID="02F7-B675"
ELEMENTS_UUID="B896919A969159A8"

# Android drive UUID varies — accept as arg or detect
if [[ -z "$ANDROID_UUID" ]]; then
    echo "NOTE: Android USB drive UUID not provided."
    echo "      Detect with: sudo blkid | grep -i android"
    echo "      Re-run with: ANDROID_UUID=XXXX-XXXX bash -s < setup-nas-mounts.sh"
    echo ""
fi

# === Install filesystem packages ===
echo "[1/3] Installing filesystem packages..."
sudo apt install -y exfat-fuse ntfs-3g

# === Create mount directories ===
echo "[2/3] Creating mount directories..."
sudo mkdir -p /mnt/nas/t7 /mnt/nas/elements /mnt/nas/android

# === Add fstab entries ===
echo "[3/3] Adding fstab entries..."

add_fstab_entry() {
    local uuid="$1" mountpoint="$2" fstype="$3"
    local line="UUID=${uuid}  ${mountpoint}  ${fstype}  defaults,nofail,uid=1000,gid=1000  0  0"
    if grep -q "$uuid" /etc/fstab; then
        echo "  - Already in fstab: $mountpoint"
    else
        echo "$line" | sudo tee -a /etc/fstab > /dev/null
        echo "  - Added: $mountpoint ($fstype, UUID=$uuid)"
    fi
}

# Add NAS drives header if not present
if ! grep -q '# NAS USB Drives' /etc/fstab; then
    echo "" | sudo tee -a /etc/fstab > /dev/null
    echo "# NAS USB Drives" | sudo tee -a /etc/fstab > /dev/null
fi

add_fstab_entry "$T7_UUID" "/mnt/nas/t7" "exfat"
add_fstab_entry "$ELEMENTS_UUID" "/mnt/nas/elements" "ntfs-3g"

if [[ -n "$ANDROID_UUID" ]]; then
    add_fstab_entry "$ANDROID_UUID" "/mnt/nas/android" "exfat"
fi

# === Mount all ===
sudo mount -a 2>/dev/null || true

echo ""
echo "=== NAS Mount Setup Complete ==="
echo "  - T7 SSD:     /mnt/nas/t7       (exfat, UUID=$T7_UUID)"
echo "  - Elements:   /mnt/nas/elements  (ntfs-3g, UUID=$ELEMENTS_UUID)"
if [[ -n "$ANDROID_UUID" ]]; then
    echo "  - Android:    /mnt/nas/android   (exfat, UUID=$ANDROID_UUID)"
else
    echo "  - Android:    /mnt/nas/android   (NOT configured — provide ANDROID_UUID)"
fi
echo ""
echo "Check: df -h /mnt/nas/*"
