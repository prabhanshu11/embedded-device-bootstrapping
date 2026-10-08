#!/bin/bash
# ONE-SHOT: format the new Seagate Portable 4 TB as ext4 (the user, 2026-10-08:
# "a FS which is safer in case of power loss ... this is a new hdd so we can format it").
#
# Guard: only runs when the disk still carries the factory NTFS volume
# (UUID D25E9F7C0132F192 on sdX2). Re-running after the format is a no-op.
# Result: GPT, one partition, ext4 label "seagate4tb", journal + write barriers (ext4 defaults),
# 1 inode per 256 KiB (media drive: fewer inodes = faster mkfs/fsck), 0.5 % root-reserved,
# fstab line "defaults,nofail,noatime" at /mnt/nas/seagate, root dir owned by pi.
#
# Usage: ssh pi@192.168.0.2 'bash -s' < bootstrap/rpi5/format-seagate-ext4.sh   (runs ON the Pi)
set -euo pipefail
export PATH=/usr/sbin:/sbin:$PATH
OLD_UUID="D25E9F7C0132F192"
MNT=/mnt/nas/seagate
LABEL=seagate4tb

# blkid must run as root on the Pi: unprivileged it answers from a stale cache / nothing (seen 2026-10-08)
part=$(sudo blkid -U "$OLD_UUID" 2>/dev/null || true)
newpart=$(sudo blkid -L "$LABEL" 2>/dev/null || true)
if [[ -n "$part" ]]; then
    disk=/dev/$(lsblk -no PKNAME "$part")
    echo "Factory volume $part on $disk ($(lsblk -dno SIZE "$disk"))"
    lsblk -o NAME,SIZE,FSTYPE,LABEL,MOUNTPOINT "$disk"

    echo "[1/5] Unmounting $MNT (if mounted)"
    mountpoint -q "$MNT" && sudo umount "$MNT"
    grep -q "$OLD_UUID" /etc/fstab && sudo sed -i "/$OLD_UUID/d" /etc/fstab

    echo "[2/5] New GPT + one partition"
    sudo wipefs -a "$disk" >/dev/null
    sudo parted -s "$disk" mklabel gpt mkpart "$LABEL" ext4 1MiB 100%
    sudo partprobe "$disk"; sleep 2
    newpart="${disk}1"

    echo "[3/5] mkfs.ext4 $newpart (a few minutes on USB 2)"
    sudo mkfs.ext4 -q -L "$LABEL" -m 0.5 -i 262144 "$newpart"
elif [[ -n "$newpart" ]]; then
    echo "Already formatted: $newpart is ext4 '$LABEL' - only (re)doing fstab + mount."
else
    echo "Factory NTFS volume $OLD_UUID not found and no '$LABEL' volume either - refusing to guess." >&2
    exit 1
fi
uuid=$(sudo blkid -s UUID -o value "$newpart")
[[ -n "$uuid" ]] || { echo "could not read the UUID of $newpart" >&2; exit 1; }
echo "  - UUID=$uuid"

echo "[4/5] fstab + mount"
sudo mkdir -p "$MNT"
sudo sed -i "\#^UUID=\s\+${MNT}#d" /etc/fstab            # drop a broken empty-UUID line if present
if ! grep -q "UUID=${uuid}" /etc/fstab; then
    echo "UUID=${uuid}  ${MNT}  ext4  defaults,nofail,noatime  0  2" | sudo tee -a /etc/fstab >/dev/null
fi
mountpoint -q "$MNT" || sudo mount "$MNT"
sudo chown pi:pi "$MNT"
mkdir -p "$MNT/TV Series/Star Trek"

echo "[5/5] Done"
df -h "$MNT" | tail -1
echo "NOTE: put UUID=$uuid into bootstrap/rpi5/setup-nas-mounts.sh (SEAGATE_UUID, ext4)."
