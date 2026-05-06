# Embedded Device Bootstrapping

Shell scripts and config templates for flashing, bootstrapping, and deploying to Raspberry Pi devices.

## Scope

This repo contains **host-side tooling** (flash SD cards, configure networks, deploy code) and **device-side bootstrap scripts** (first-boot setup, tunnels, hotspot). Application code (Python services, Rust programs) stays in their respective repos.

## Supported Devices

| Device | Profile | Address | Hostname | Use Case |
|--------|---------|---------|----------|----------|
| Raspberry Pi 5 | `rpi5` | `rpi5` (Tailscale) / 192.168.0.2 (LAN) | rpi5 | DNS-only Pi-hole on Archer-C6 network, NAS host |
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

# RPi5 (Tailscale — works from anywhere on the tailnet)
ssh pi@rpi5

# RPi5 (LAN fallback when on Archer-C6 home WiFi)
ssh pi@192.168.0.2
```

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
