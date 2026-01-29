#!/bin/bash
# Common base setup for Raspberry Pi devices
#
# Usage: ssh pi@<IP> 'bash -s' < bootstrap/common/base-setup.sh
#
# Installs: system updates, python3, venv, git, curl, ssh key generation
# Runs ON the Pi (not on host).

set -e

echo "=== Base Pi Setup ==="
echo ""

# Update system
echo "[1/4] Updating system packages..."
sudo apt update && sudo apt upgrade -y

# Install common dependencies
echo "[2/4] Installing dependencies..."
sudo apt install -y \
    python3 \
    python3-pip \
    python3-venv \
    sqlite3 \
    git \
    curl \
    wget

# Generate SSH key if it doesn't exist
echo "[3/4] Setting up SSH key..."
if [ ! -f ~/.ssh/id_ed25519 ]; then
    ssh-keygen -t ed25519 -f ~/.ssh/id_ed25519 -N "" -C "pi@$(hostname)"
    echo "  Generated new SSH key:"
    cat ~/.ssh/id_ed25519.pub
else
    echo "  SSH key already exists:"
    cat ~/.ssh/id_ed25519.pub
fi

# Basic security
echo "[4/4] Configuring basic security..."
# Disable password auth (rely on SSH keys after initial setup)
# Uncomment the line below after copying your SSH key to the Pi:
# sudo sed -i 's/#PasswordAuthentication yes/PasswordAuthentication no/' /etc/ssh/sshd_config

echo ""
echo "=== Base Setup Complete ==="
echo ""
echo "Installed: python3, pip, venv, sqlite3, git, curl, wget"
echo "SSH public key: ~/.ssh/id_ed25519.pub"
echo ""
echo "Next: Run device-specific setup scripts or deploy/deploy.sh"
