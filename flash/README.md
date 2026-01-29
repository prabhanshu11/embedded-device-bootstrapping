# SD Card Flashing

Flash Raspberry Pi OS Lite (Bookworm arm64) to SD cards with device-specific configuration.

## Profiles

| Profile | USB Gadget | Static IP | Use Case |
|---------|-----------|-----------|----------|
| `pi-zero-2w` | dwc2 + g_ether | 10.55.0.2 (usb0) | Pi Zero 2W over USB |
| `rpi5` | No | Optional (eth0) | RPi5 on ethernet/WiFi |
| `generic` | No | No | Any Pi, minimal config |

## Direct Usage (requires root)

```bash
# Pi Zero 2W with WiFi
sudo ./flash/flash-sd.sh \
  --device /dev/mmcblk0 \
  --profile pi-zero-2w \
  --wifi "MyNetwork" "mypassword"

# RPi5 with static IP
sudo ./flash/flash-sd.sh \
  --device /dev/mmcblk0 \
  --profile rpi5 \
  --static-ip 192.168.29.10/24 \
  --wifi "MyNetwork" "mypassword"
```

## Docker Usage (no repeated sudo)

```bash
# Build (one-time)
docker build -t pi-flasher flash/

# Flash
docker run --rm -it --privileged \
  -v /dev:/dev \
  -v ~/Downloads:/downloads \
  pi-flasher \
  flash-sd --device /dev/mmcblk0 --profile pi-zero-2w --force
```

The Docker container caches the downloaded image in the mounted `/downloads` volume.

## What Each Profile Configures

All profiles:
- SSH enabled (empty `ssh` file in boot)
- User `pi` with specified password (Bookworm `userconf`)
- Hostname set in `/etc/hostname` and `/etc/hosts`
- WiFi via NetworkManager `.nmconnection` (if `--wifi` provided)

`pi-zero-2w` additionally:
- `dtoverlay=dwc2` in `config.txt`
- `modules-load=dwc2,g_ether` in `cmdline.txt`
- Static IP 10.55.0.2/24 on `usb0` via NetworkManager

`rpi5` additionally:
- Static ethernet IP on `eth0` (if `--static-ip` provided)
