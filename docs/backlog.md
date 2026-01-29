# Backlog: RPi5 Bootstrap Scripts

Services and configs currently running on RPi5 that need bootstrap scripts for disaster recovery.

## Priority 1: Network Stack (Hotspot Infrastructure)

### ~~Update hotspot-setup for wlan0 direct mode~~ DONE
- **Script**: `bootstrap/rpi5/setup-hotspot.sh`
- hostapd runs directly on `wlan0` (not virtual `wlan0_ap`)
- Disables `wpa_supplicant@wlan0` (conflicts with hostapd)
- Trade-off: no WiFi client fallback — Pi relies on eth0

### ~~Add uplink-monitor service~~ DONE
- **Script**: `bootstrap/rpi5/setup-uplink-monitor.sh`
- Installs script + systemd service

### ~~Add iptables NAT + forwarding rules~~ DONE
- **Script**: `bootstrap/rpi5/setup-nat.sh`
- NAT: wlan0 → eth0 (MASQUERADE)
- Persisted via netfilter-persistent

### Add wpa_supplicant template for home WiFi
- **What**: `wpa_supplicant-wlan0.conf` connects wlan0 to home WiFi as internet uplink
- **Secrets**: SSID + PSK → `%%WIFI_CLIENT_SSID%%`, `%%WIFI_CLIENT_PSK%%`
- **Action**: Add `bootstrap/rpi5/wpa_supplicant.conf.template`
- **Note**: Only useful if reverting to virtual AP mode (wlan0_ap for AP, wlan0 for client)

## Priority 2: NAS Stack

### ~~Add NAS USB drive fstab entries~~ DONE
- **Script**: `bootstrap/rpi5/setup-nas-mounts.sh`
- T7 SSD (exfat), Elements HDD (ntfs-3g), Android USB (exfat, UUID as arg)

### ~~Add Samba setup~~ DONE
- **Script**: `bootstrap/rpi5/setup-samba.sh`
- [nas] share at /mnt/nas for pi user

### ~~Add Filebrowser setup~~ DONE
- **Script**: `bootstrap/rpi5/setup-filebrowser.sh`
- Web UI at :8080 serving /mnt/nas

## Priority 3: DNS / Ad-blocking

### ~~Pi-hole setup~~ DONE
- **Script**: `bootstrap/rpi5/setup-pihole.sh`
- Runs Pi-hole installer, then configures DHCP for hotspot subnet (192.168.50.x)

## Priority 4: Application Services

### pibox-server (separate repo)
- **What**: WebSocket server for NAS management, depends on Filebrowser
- **Binary**: `/usr/local/bin/pibox-server` (Rust, cross-compiled)
- **Action**: Stays in pibox repo — deploy via `deploy/deploy.sh` after Filebrowser is up

## Remaining TODO

- [ ] Add `wpa_supplicant.conf.template` (low priority — only needed for WiFi client fallback)
- [ ] Update iptables rules on running Pi (still reference old wlan0_ap)
- [ ] Update `hotspot-setup.service` on Pi (still creates wlan0_ap, should assign IP to wlan0)
- [ ] ShellCheck all bootstrap scripts

## Full Recovery Sequence

After flashing with `flash/flash-sd.sh --profile rpi5`:

```bash
# 1. Base setup
ssh pi@IP 'bash -s' < bootstrap/common/base-setup.sh

# 2. Network stack
ssh pi@IP 'WIFI_SSID=x WIFI_PASSPHRASE=y bash -s' < bootstrap/rpi5/setup-hotspot.sh
ssh pi@IP 'bash -s' < bootstrap/rpi5/setup-nat.sh
ssh pi@IP 'bash -s' < bootstrap/rpi5/setup-uplink-monitor.sh

# 3. DNS + DHCP
ssh pi@IP 'bash -s' < bootstrap/rpi5/setup-pihole.sh

# 4. NAS stack
ssh pi@IP 'bash -s' < bootstrap/rpi5/setup-nas-mounts.sh
ssh pi@IP 'SAMBA_PASSWORD=secret bash -s' < bootstrap/rpi5/setup-samba.sh
ssh pi@IP 'bash -s' < bootstrap/rpi5/setup-filebrowser.sh

# 5. Apps (from their own repos)
./deploy/deploy.sh --host pi@IP --service pibox ...
```
