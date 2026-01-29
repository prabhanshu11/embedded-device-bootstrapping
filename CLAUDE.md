# Embedded Device Bootstrapping

Shell scripts and config templates for flashing, bootstrapping, and deploying to Raspberry Pi devices.

## Scope

This repo contains **host-side tooling** (flash SD cards, configure networks, deploy code) and **device-side bootstrap scripts** (first-boot setup, tunnels, hotspot). Application code (Python services, Rust programs) stays in their respective repos.

## Supported Devices

| Device | Profile | IP | Hostname | Use Case |
|--------|---------|-----|----------|----------|
| Raspberry Pi 5 | `rpi5` | 192.168.29.10 | pi-hub | WiFi hotspot, orchestrator |
| Pi Zero 2W | `pi-zero-2w` | 10.55.0.2 (USB) | pi-keyboard | BT HID keyboard, satellite |

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

# RPi5 (via home network from desktop)
ssh pi@192.168.29.10

# RPi5 (via desktop SSH hop from laptop)
ssh desktop "ssh pi@192.168.29.10"
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
