# Embedded Device Bootstrapping

Shell scripts and config templates for flashing, bootstrapping, and deploying to Raspberry Pi devices.

## Scope

This repo contains **host-side tooling** (flash SD cards, configure networks, deploy code) and **device-side bootstrap scripts** (first-boot setup, tunnels, hotspot). Application code (Python services, Rust programs) stays in their respective repos.

## Supported Devices

| Device | Profile | Address | Hostname | Use Case |
|--------|---------|---------|----------|----------|
| Raspberry Pi 5 | `rpi5` | 192.168.0.2 (LAN) / 10.77.0.4 (wg-vdi VPS relay, since 2026-11-09) / `rpi5` 100.124.200.77 (Tailscale, backup) | rpi5 | DNS-only Pi-hole on Archer-C6 network, NAS host |
| Pi Zero 2W | `pi-zero-2w` | 10.55.0.2 (USB) | pi-keyboard | BT HID keyboard, satellite |

**Pi5 architecture changed 2026-04-10.** Old `192.168.29.10` (Jio LAN) and `192.168.50.x` (Pi5 hostapd) addresses are dead. Pi5 now sits at 192.168.0.2 on the Archer C6 network as DNS-only, with Tailscale `rpi5` as the canonical name. See [reference_pi5_ssh.md](../../.claude/projects/-home-prabhanshu-Programs/memory/reference_pi5_ssh.md). The `setup-hotspot.sh`, `setup-nat.sh`, and DHCP portions of `setup-pihole.sh` describe the OBSOLETE pre-Apr-10 architecture and should not be run on a fresh flash without architecture review.

## Pipeline

```
flash/flash-sd.sh          # 1. Flash SD card with OS + device config
  ↓
bootstrap/common/           # 2. First-boot: apt, python, ssh keys
bootstrap/{device}/         # 2b. Device-specific: hotspot, USB gadget
  ↓
deploy/deploy.sh            # 3. Push app code + systemd service
```

## SSH Access

```bash
# Pi Zero 2W (via USB gadget from laptop)
ssh pi@10.55.0.2

# RPi5 from the laptop/desktop, ANY network: one name, channel picked in his order
# (LAN -> VPS relay 10.77.0.4 -> relay via desktop -> Tailscale -> Tailscale via desktop)
# by local-bootstrapping dotfiles/local-bin/home-route + dotfiles/ssh/home-routes.conf
ssh pi5            # also: ssh nas; sftp://pi5/... in Thunar; `home-route status`

# RPi5 direct, when you know the network
ssh pi@192.168.0.2      # home LAN (Archer C6)
ssh pi@10.77.0.4        # wg-vdi relay (self-hosted, hub on the VPS)
ssh pi@rpi5             # Tailscale (backup only)
```

**wg-vdi on the Pi (2026-11-09, his yes in Kanpur):** the Pi is a member of the self-hosted
WireGuard relay at 10.77.0.4, so the NAS is reachable away from home without the desktop being
on. Set up by local-bootstrapping `scripts/vdi/setup-wg-vdi.sh pi` (wireguard-tools from apt,
`wg-quick@wg-vdi`, `wg-vdi-heal.timer` every 30 s); the hub peer is in vps_bootstrap
`wireguard/peers.conf`. A fresh flash re-runs that script and puts the new public key there.

## Networking: Tailscale Stays Installed (the user, 2026-10-03)

- Tailscale is **not preferred** for new features: reach devices over the home LAN first. We are actively
  looking for a private, self-hosted alternative.
- **Tailscale MUST remain installed and logged in on every device this repo provisions** (RPi5, Pi Zero 2W,
  future boards). Never uninstall, disable or `tailscale logout`; never drop it from a flash/bootstrap script.
  His words: every alternative network profile tried so far hit issues and we had to fall back to Tailscale.

## Secrets

All secrets use `%%PLACEHOLDER%%` syntax in templates. Provide via:
1. `pass` manager: `pass show embedded/rpi5-wifi-passphrase`
2. Environment variables: `WIFI_PASSPHRASE=secret ./script.sh`
3. Interactive prompt (scripts ask if env var not set)

Never commit plaintext passwords. Config templates use `.template` suffix.

## Related Repos

- `esp32-bt-hid` - BT keyboard Python code + ESP32 firmware (deployed via this repo's deploy.sh)
- `life-dashboard` - Calendar API Python code (deployed via this repo's deploy.sh)
- `pibox` - Rust workspace for Pi hardware control (separate repo)
- `pi-flasher` - Legacy Docker flasher (superseded by flash/ in this repo)

## Development

```bash
# Check scripts for errors
shellcheck flash/flash-sd.sh bootstrap/**/*.sh deploy/deploy.sh

# Build Docker flasher
docker build -t pi-flasher flash/

# Flash a Pi Zero 2W SD card
sudo ./flash/flash-sd.sh --device /dev/mmcblk0 --profile pi-zero-2w
```
