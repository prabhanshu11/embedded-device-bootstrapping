#!/bin/bash
# Set up persistent autossh reverse tunnel from Pi to VPS
#
# Usage: ssh pi@<IP> 'bash -s' < bootstrap/common/tunnel-setup.sh
#   Or: ssh pi@<IP> 'bash -s -- --vps-host 72.60.218.33 --remote-port 8082 --local-port 8080 --service-name my-tunnel'
#
# Creates a systemd service that maintains a persistent SSH tunnel
# from the Pi to the VPS, allowing external access to a local service.
#
# Architecture: Pi:LOCAL_PORT <-- SSH tunnel --> VPS:REMOTE_PORT
#
# Runs ON the Pi (not on host).

set -e

# === Defaults ===
VPS_HOST="72.60.218.33"
VPS_USER="root"
REMOTE_PORT="8082"
LOCAL_PORT="8080"
SERVICE_NAME="reverse-tunnel"
SSH_KEY="$HOME/.ssh/id_ed25519_vps"

# === Parse Arguments ===
while [[ $# -gt 0 ]]; do
    case "$1" in
        --vps-host)    VPS_HOST="$2"; shift 2 ;;
        --vps-user)    VPS_USER="$2"; shift 2 ;;
        --remote-port) REMOTE_PORT="$2"; shift 2 ;;
        --local-port)  LOCAL_PORT="$2"; shift 2 ;;
        --service-name) SERVICE_NAME="$2"; shift 2 ;;
        --ssh-key)     SSH_KEY="$2"; shift 2 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

echo "=== Reverse Tunnel Setup ==="
echo ""
echo "VPS:         $VPS_USER@$VPS_HOST"
echo "Tunnel:      VPS:$REMOTE_PORT -> Pi:$LOCAL_PORT"
echo "Service:     $SERVICE_NAME"
echo "SSH key:     $SSH_KEY"
echo ""

# Install autossh
echo "[1/4] Installing autossh..."
sudo apt install -y autossh

# Generate VPS SSH key if needed
echo "[2/4] Setting up SSH key for VPS..."
if [ ! -f "$SSH_KEY" ]; then
    ssh-keygen -t ed25519 -f "$SSH_KEY" -N "" -C "$(hostname)-tunnel"
    echo ""
    echo ">>> ACTION REQUIRED <<<"
    echo "Add this public key to VPS authorized_keys:"
    echo ""
    cat "${SSH_KEY}.pub"
    echo ""
    echo "On VPS, run:"
    echo "  echo '$(cat "${SSH_KEY}.pub")' >> ~/.ssh/authorized_keys"
    echo ""
    read -p "Press Enter after you've added the key to VPS..."
else
    echo "  SSH key already exists: $SSH_KEY"
fi

# Test SSH connection
echo "[3/4] Testing SSH connection to VPS..."
if ssh -i "$SSH_KEY" -o ConnectTimeout=10 -o StrictHostKeyChecking=accept-new "$VPS_USER@$VPS_HOST" "echo 'VPS connection OK'"; then
    echo "  SSH to VPS successful!"
else
    echo "ERROR: Cannot connect to VPS"
    echo "Make sure the SSH key is added to VPS authorized_keys"
    exit 1
fi

# Create systemd service
echo "[4/4] Installing tunnel service..."
sudo tee "/etc/systemd/system/${SERVICE_NAME}.service" > /dev/null << EOF
[Unit]
Description=Reverse SSH Tunnel to VPS ($SERVICE_NAME)
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$(whoami)
Environment="AUTOSSH_GATETIME=0"
ExecStart=/usr/bin/autossh -M 0 -N \
  -o "ServerAliveInterval=30" \
  -o "ServerAliveCountMax=3" \
  -o "ExitOnForwardFailure=yes" \
  -o "StrictHostKeyChecking=accept-new" \
  -i $SSH_KEY \
  -R ${REMOTE_PORT}:127.0.0.1:${LOCAL_PORT} \
  ${VPS_USER}@${VPS_HOST}
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable "$SERVICE_NAME"

echo ""
echo "=== Tunnel Setup Complete ==="
echo ""
echo "Service: $SERVICE_NAME"
echo "Tunnel:  VPS:$REMOTE_PORT -> localhost:$LOCAL_PORT"
echo ""
echo "Commands:"
echo "  sudo systemctl start $SERVICE_NAME    # Start tunnel"
echo "  sudo systemctl status $SERVICE_NAME   # Check status"
echo "  sudo journalctl -u $SERVICE_NAME -f   # View logs"
