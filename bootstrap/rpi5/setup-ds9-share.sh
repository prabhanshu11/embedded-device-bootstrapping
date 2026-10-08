#!/bin/bash
# Guest, read-only Samba share [DS9] of the Star Trek DS9 laserdisc rips on the Seagate 4 TB,
# for the iPhone (VLC / Files) and the laptop (cifs automount, see local-bootstrapping).
# Mirrors the laptop's [DS9] share (local-bootstrapping/system/samba/smb.conf) that it replaces.
#
# Also installs the Bonjour advert so "rpi5" shows under Shared on iOS without typing an address.
# Idempotent. Runs ON the Pi:
#   ssh pi@192.168.0.2 'bash -s' < bootstrap/rpi5/setup-ds9-share.sh
#   (the avahi advert is sent inline below, so no second file transfer is needed)
set -euo pipefail
SHARE_DIR="/mnt/nas/seagate/TV Series/Star Trek/star-trek-ds9-laserdisc-720p-24"

echo "=== RPi5 [DS9] share ==="
[[ -d "$SHARE_DIR" ]] || { echo "missing: $SHARE_DIR" >&2; exit 1; }

echo "[1/3] smb.conf [DS9]"
if grep -q '^\[DS9\]' /etc/samba/smb.conf; then
    echo "  - [DS9] already present"
else
    sudo tee -a /etc/samba/smb.conf >/dev/null <<SMB

[DS9]
   comment = Star Trek DS9 laserdisc 720p (read-only, guest)
   path = $SHARE_DIR
   browseable = yes
   read only = yes
   guest ok = yes
   guest only = yes
   force user = pi
   # fast directory listing for iOS VLC / Files and the laptop: no DOS attrs / EAs to look up per file
   store dos attributes = no
   ea support = no
   map archive = no
   map hidden = no
   map system = no
SMB
    echo "  - [DS9] appended"
fi
sudo testparm -s /etc/samba/smb.conf >/dev/null 2>&1 || { echo "smb.conf failed testparm" >&2; exit 1; }

echo "[2/3] Bonjour advert (_smb._tcp)"
sudo install -d /etc/avahi/services
sudo tee /etc/avahi/services/smb.service >/dev/null <<'AVAHI'
<?xml version="1.0" standalone='no'?>
<!DOCTYPE service-group SYSTEM "avahi-service.dtd">
<!-- Advertise the Samba share over Bonjour so the iPhone Files app / VLC list this Pi under
     "Shared". Source: embedded-device-bootstrapping/configs/rpi5/avahi-smb.service -->
<service-group>
  <name replace-wildcards="yes">%h</name>
  <service>
    <type>_smb._tcp</type>
    <port>445</port>
  </service>
  <service>
    <type>_device-info._tcp</type>
    <port>0</port>
    <txt-record>model=MacSamba</txt-record>
  </service>
</service-group>
AVAHI

echo "[3/3] restart smbd"
sudo systemctl restart smbd
sudo systemctl is-active --quiet avahi-daemon || sudo systemctl restart avahi-daemon
echo "smbd: $(systemctl is-active smbd)   files in share: $(find "$SHARE_DIR" -type f | wc -l)"
echo "iPhone VLC / Files: smb://rpi5.local (or 192.168.0.2) -> DS9, Guest, no password"
