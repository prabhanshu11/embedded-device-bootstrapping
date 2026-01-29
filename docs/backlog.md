# Backlog — embedded-device-bootstrapping

## RPi5 Services to Bootstrap

These services are currently running on the RPi5 but have no bootstrap script in this repo yet. Adding them enables full disaster recovery (re-flash SD card → run scripts → everything works).

### Network Stack

- [ ] **Fix hotspot-setup oneshot service**
  - Current `setup-hotspot.sh` in repo installs hostapd/dnsmasq but misses the `hotspot-setup.service` systemd oneshot
  - Oneshot creates virtual `wlan0_ap` interface (`iw dev wlan0 interface add wlan0_ap type __ap`) and assigns `192.168.50.1/24`
  - Must run Before=hostapd.service
  - Note: current repo dnsmasq.conf uses `192.168.4.x` range but live Pi uses `192.168.50.1/24` — reconcile

- [ ] **uplink-monitor service**
  - Script: `/usr/local/bin/uplink-monitor.sh` — auto-switches default route between eth0 and wlan0 every 10s
  - Systemd unit: `uplink-monitor.service` (Type=simple, Restart=always)
  - Add as `bootstrap/rpi5/uplink-monitor.sh` + service template

- [ ] **iptables NAT + forwarding rules**
  - FORWARD: `wlan0_ap → eth0`, `wlan0_ap → wlan0` (+ RELATED,ESTABLISHED return)
  - NAT MASQUERADE: on `eth0` and `wlan0`
  - Persisted via `netfilter-persistent` (`apt install iptables-persistent netfilter-persistent`)
  - Add as `bootstrap/rpi5/setup-firewall.sh`

- [ ] **wpa_supplicant template for home WiFi**
  - Current: `/etc/wpa_supplicant/wpa_supplicant-wlan0.conf` with plaintext PSK
  - Add template: `bootstrap/rpi5/wpa_supplicant.conf.template` with `%%WIFI_PSK%%` placeholder
  - Country: IN, key_mgmt: WPA-PSK

### NAS Stack

- [ ] **NAS drive mounts (fstab entries)**
  - Samsung T7 (exfat): `UUID=02F7-B675 /mnt/nas/t7 exfat defaults,nofail,uid=1000,gid=1000 0 0`
  - WD Elements (ntfs-3g): `UUID=B896919A969159A8 /mnt/nas/elements ntfs-3g defaults,nofail,uid=1000,gid=1000 0 0`
  - Android phone (exfat): mounted at `/mnt/nas/android` (intermittent)
  - Need: `mkdir -p /mnt/nas/{t7,elements,android}`, append to fstab, install `ntfs-3g exfat-utils`
  - Add as `bootstrap/rpi5/setup-nas-mounts.sh`

- [ ] **Samba file sharing**
  - Custom section in `/etc/samba/smb.conf`: `[nas]` share at `/mnt/nas`, valid users = pi
  - Install: `apt install samba`
  - Set samba password: `smbpasswd -a pi`
  - Add as `bootstrap/rpi5/setup-samba.sh` + `bootstrap/rpi5/smb-nas.conf` (appended to default config)

- [ ] **Filebrowser web UI**
  - Binary: `/usr/local/bin/filebrowser` v2.55.0
  - Serves `/mnt/nas` on `0.0.0.0:8080`, DB at `/home/pi/.config/filebrowser/filebrowser.db`
  - Install: download binary from GitHub releases
  - Systemd unit: `filebrowser.service` (User=pi)
  - Add as `bootstrap/rpi5/setup-filebrowser.sh`

### Application Services (NOT in this repo — reference only)

- [ ] **pibox-server** — stays in `pibox` repo (Rust binary at `/usr/local/bin/pibox-server`)
  - Depends on filebrowser, listens on `0.0.0.0:9280`
  - Deploy via cross-compilation + SCP from pibox repo

- [ ] **Pi-hole** — use official installer (`curl -sSL https://install.pi-hole.net | bash`)
  - DNS ad-blocking, runs as `pihole-FTL` service
  - Current version: Core v6.3, Web v6.4, FTL v6.4.1
  - Consider: document post-install config (upstream DNS, blocklists) but don't script the install itself

### Future

- [ ] **RAID setup** for NAS drives (research needed — mdadm vs btrfs vs mergerfs+snapraid)
- [ ] **Google Takeout data management** — browse, validate against cloud, selective deletion
- [ ] **Reverse SSH tunnel** — if RPi5 needs external access (use `bootstrap/common/tunnel-setup.sh`)
