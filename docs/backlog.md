# Backlog: RPi5 Bootstrap Scripts

Services and configs currently running on RPi5 that need bootstrap scripts for disaster recovery.

## Priority 1: Network Stack (Hotspot Infrastructure)

### Fix hotspot-setup oneshot service
- **What**: `hotspot-setup.service` is a oneshot that runs before hostapd
- **Does**: Creates virtual `wlan0_ap` interface, assigns `192.168.50.1/24`, brings it up
- **Script on Pi**: `/usr/local/bin/setup-hotspot.sh`
- **Action**: Add `bootstrap/rpi5/hotspot-interface.sh` + systemd unit
- **Current content**:
  ```bash
  iw dev wlan0 interface add wlan0_ap type __ap
  ip addr add 192.168.50.1/24 dev wlan0_ap
  ip link set wlan0_ap up
  ```

### Add uplink-monitor service
- **What**: `uplink-monitor.service` auto-switches default route between eth0 and wlan0
- **Does**: Checks every 10s if eth0 has carrier+IP, switches default route accordingly
- **Script on Pi**: `/usr/local/bin/uplink-monitor.sh`
- **Action**: Add `bootstrap/rpi5/uplink-monitor.sh` + systemd unit

### Add iptables NAT + forwarding rules
- **What**: Hotspot clients get internet via eth0 or wlan0
- **Rules**:
  - FORWARD: `wlan0_ap → eth0`, `wlan0_ap → wlan0` (+ ESTABLISHED return)
  - NAT POSTROUTING: MASQUERADE on eth0 and wlan0
- **Persisted via**: `netfilter-persistent`
- **Action**: Add `bootstrap/rpi5/setup-nat.sh` (install netfilter-persistent, write rules, save)

### Add wpa_supplicant template for home WiFi
- **What**: `wpa_supplicant-wlan0.conf` connects wlan0 to home WiFi as internet uplink
- **Secrets**: SSID + PSK → `%%WIFI_CLIENT_SSID%%`, `%%WIFI_CLIENT_PSK%%`
- **Action**: Add `bootstrap/rpi5/wpa_supplicant.conf.template`

## Priority 2: NAS Stack

### Add NAS USB drive fstab entries
- **What**: Three USB drives auto-mounted at boot
- **Drives**:
  - Samsung T7 SSD (exfat) → `/mnt/nas/t7` (UUID=02F7-B675)
  - WD Elements HDD (ntfs-3g) → `/mnt/nas/elements` (UUID=B896919A969159A8)
  - Android drive (exfat) → `/mnt/nas/android` (varies)
- **Packages**: `exfat-fuse ntfs-3g`
- **Action**: Add `bootstrap/rpi5/setup-nas-mounts.sh` (install packages, mkdir, add fstab entries with `nofail`)
- **Note**: UUIDs are drive-specific — script should detect or accept as args

### Add Samba setup
- **What**: SMB file sharing of `/mnt/nas` for `pi` user
- **Packages**: `samba`
- **Config**: Custom `[nas]` share section in `smb.conf`
- **Action**: Add `bootstrap/rpi5/setup-samba.sh` + `smb-nas.conf.template`
- **Sets up**: `smbpasswd` for pi user (interactive or from env)

### Add Filebrowser setup
- **What**: Web UI for NAS files at `0.0.0.0:8080`, serves `/mnt/nas`
- **Binary**: `/usr/local/bin/filebrowser` v2.55.0 (installed via curl script)
- **DB**: `/home/pi/.config/filebrowser/filebrowser.db`
- **Action**: Add `bootstrap/rpi5/setup-filebrowser.sh` (download binary, create systemd unit, enable)
- **Install**: `curl -fsSL https://raw.githubusercontent.com/filebrowser/get/master/get.sh | bash`

## Priority 3: DNS / Ad-blocking

### Pi-hole (deferred)
- **What**: DNS ad-blocking via `pihole-FTL`
- **Install**: Has its own installer (`curl -sSL https://install.pi-hole.net | bash`)
- **Action**: Document install command + post-install config in a setup script
- **Note**: Pi-hole installer is interactive; bootstrap script would call it and then apply config

## Priority 4: Application Services

### pibox-server (separate repo)
- **What**: WebSocket server for NAS management, depends on Filebrowser
- **Binary**: `/usr/local/bin/pibox-server` (Rust, cross-compiled)
- **Action**: Stays in pibox repo — deploy via `deploy/deploy.sh` after Filebrowser is up
- **Systemd unit**: Already documented above, deploy script handles it

## Implementation Order

1. `setup-nat.sh` + iptables rules (hotspot won't route without this)
2. `hotspot-interface.sh` + oneshot service (fix current partial coverage)
3. `wpa_supplicant.conf.template` (WiFi uplink)
4. `uplink-monitor.sh` + service (auto-switch)
5. `setup-nas-mounts.sh` (fstab + packages)
6. `setup-samba.sh` (NAS sharing)
7. `setup-filebrowser.sh` (web UI)
8. Pi-hole documentation (last — least critical for recovery)

## Full Recovery Sequence

After flashing with `flash/flash-sd.sh --profile rpi5`:

```bash
# 1. Base setup
ssh pi@IP 'bash -s' < bootstrap/common/base-setup.sh

# 2. Network stack
ssh pi@IP 'WIFI_CLIENT_SSID=x WIFI_CLIENT_PSK=y bash -s' < bootstrap/rpi5/setup-wpa-client.sh
ssh pi@IP 'bash -s' < bootstrap/rpi5/hotspot-interface.sh   # installs oneshot service
ssh pi@IP 'WIFI_SSID=x WIFI_PASSPHRASE=y bash -s' < bootstrap/rpi5/setup-hotspot.sh
ssh pi@IP 'bash -s' < bootstrap/rpi5/setup-nat.sh
ssh pi@IP 'bash -s' < bootstrap/rpi5/setup-uplink-monitor.sh

# 3. NAS stack
ssh pi@IP 'bash -s' < bootstrap/rpi5/setup-nas-mounts.sh
ssh pi@IP 'bash -s' < bootstrap/rpi5/setup-samba.sh
ssh pi@IP 'bash -s' < bootstrap/rpi5/setup-filebrowser.sh

# 4. Apps (from their own repos)
./deploy/deploy.sh --host pi@IP --service pibox ...

# 5. Optional
# Pi-hole: curl -sSL https://install.pi-hole.net | bash
```
