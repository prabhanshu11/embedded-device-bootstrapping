#!/bin/bash
# Set up Samba NAS share for /mnt/nas
#
# Creates a [nas] share accessible by the pi user.
#
# Usage: ssh pi@<IP> 'bash -s' < bootstrap/rpi5/setup-samba.sh
#   Or: ssh pi@<IP> 'SAMBA_PASSWORD=secret bash -s' < bootstrap/rpi5/setup-samba.sh
#
# Runs ON the Pi (not on host).

set -e

echo "=== RPi5 Samba Setup ==="
echo ""

# === Get password ===
if [[ -z "$SAMBA_PASSWORD" ]]; then
    read -sp "Samba password for pi user: " SAMBA_PASSWORD
    echo
fi

# === Install samba ===
echo "[1/3] Installing samba..."
sudo apt install -y samba

# === Configure share ===
echo "[2/3] Configuring [nas] share..."

if grep -q '\[nas\]' /etc/samba/smb.conf; then
    echo "  - [nas] share already exists in smb.conf"
else
    sudo tee -a /etc/samba/smb.conf > /dev/null << 'EOF'

[nas]
   comment = NAS Storage
   path = /mnt/nas
   browseable = yes
   read only = no
   guest ok = no
   valid users = pi
   create mask = 0644
   directory mask = 0755
EOF
    echo "  - Added [nas] share to smb.conf"
fi

# === Set samba password ===
echo "[3/3] Setting samba password for pi..."
(echo "$SAMBA_PASSWORD"; echo "$SAMBA_PASSWORD") | sudo smbpasswd -s -a pi

# === Enable and start ===
sudo systemctl enable smbd nmbd
sudo systemctl restart smbd nmbd

echo ""
echo "=== Samba Setup Complete ==="
echo "  - Share: \\\\$(hostname -I | awk '{print $1}')\\nas"
echo "  - User: pi"
echo "  - Path: /mnt/nas"
