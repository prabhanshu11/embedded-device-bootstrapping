#!/bin/bash
# setup-tailscale.sh — Install and configure Tailscale on Raspberry Pi
#
# Supports: Raspberry Pi OS (Debian-based)
# Safe to run multiple times (idempotent)

set -e

log() { echo "[+] $1"; }
warn() { echo "[!] $1"; }
info() { echo "[i] $1"; }

install_tailscale() {
    if command -v tailscale &>/dev/null; then
        log "Tailscale already installed: $(tailscale version | head -1)"
        return
    fi

    log "Installing Tailscale via official script..."
    curl -fsSL https://tailscale.com/install.sh | sh
}

enable_tailscale_service() {
    log "Enabling Tailscale daemon..."
    sudo systemctl enable --now tailscaled
    log "Tailscale daemon is running"
}

authenticate_tailscale() {
    if tailscale status &>/dev/null; then
        local ip=$(tailscale ip -4 2>/dev/null)
        if [[ -n "$ip" ]]; then
            log "Already connected to tailnet with IP: $ip"
            return
        fi
    fi

    info "Tailscale needs authentication."
    info "Running 'tailscale up' — follow the link to authenticate..."
    echo ""
    sudo tailscale up
    echo ""
    log "Authenticated successfully!"
}

main() {
    echo "=== Tailscale Setup (RPi) ==="
    echo ""
    install_tailscale
    enable_tailscale_service
    authenticate_tailscale
    echo ""
    tailscale status
    log "Tailscale setup complete!"
}

main "$@"
