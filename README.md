# embedded-device-bootstrapping

Shell scripts and config templates for flashing, bootstrapping, and deploying to Raspberry Pi devices (RPi5, Pi Zero 2W).

## Pipeline

```
Flash SD card → First-boot setup → Deploy application
```

### 1. Flash SD Card

Download Raspberry Pi OS, flash to SD card, and configure for the target device:

```bash
# Pi Zero 2W (USB gadget network)
sudo ./flash/flash-sd.sh --device /dev/mmcblk0 --profile pi-zero-2w

# RPi5 with WiFi and static IP
sudo ./flash/flash-sd.sh --device /dev/mmcblk0 --profile rpi5 \
  --wifi "MyNetwork" "password" --static-ip 192.168.29.10/24

# Or use Docker (no repeated sudo)
docker build -t pi-flasher flash/
docker run --rm -it --privileged -v /dev:/dev -v ~/Downloads:/downloads \
  pi-flasher flash-sd --device /dev/mmcblk0 --profile pi-zero-2w --force
```

### 2. Bootstrap Device

Run on the Pi after first boot:

```bash
# Common setup (packages, SSH keys)
ssh pi@10.55.0.2 'bash -s' < bootstrap/common/base-setup.sh

# Device-specific
ssh pi@10.55.0.2 'bash -s' < bootstrap/common/tunnel-setup.sh  # Reverse SSH tunnel
ssh pi@192.168.29.10 'WIFI_SSID=Net WIFI_PASSPHRASE=pass bash -s' < bootstrap/rpi5/setup-hotspot.sh

# Laptop-side USB network (for Pi Zero 2W)
./bootstrap/pi-zero-2w/laptop-setup.sh
```

### 3. Deploy Application

Push files and install systemd service:

```bash
./deploy/deploy.sh \
  --host pi@10.55.0.2 \
  --files "src/app.py src/config.py" \
  --service my-app \
  --exec-start "/home/pi/my-app/venv/bin/python /home/pi/my-app/app.py"
```

## Structure

```
flash/              SD card flashing (runs on host)
bootstrap/
  common/           Shared first-boot scripts (runs on device)
  rpi5/             RPi5-specific (hotspot, configs)
  pi-zero-2w/       Pi Zero 2W-specific (USB gadget network)
deploy/             Push code to devices (runs on host)
configs/            Per-device env templates
```

## Device Profiles

| Profile | USB Gadget | Default IP | Default Hostname |
|---------|-----------|------------|------------------|
| `pi-zero-2w` | Yes (dwc2, g_ether) | 10.55.0.2 | pi-keyboard |
| `rpi5` | No | User-specified | pi-hub |
| `generic` | No | DHCP | raspberrypi |
