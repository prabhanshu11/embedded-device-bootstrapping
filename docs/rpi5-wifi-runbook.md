# RPi5 WiFi Hotspot Runbook

Quick reference for diagnosing and fixing the "Prabhanshu" hotspot on the Pi 5.

## Access

```bash
ssh pi@192.168.29.10        # via ethernet (always works if Pi is on)
ssh pi@192.168.50.1          # via hotspot (only if hotspot is working)
```

## Architecture

```
Phone/Laptop --WiFi--> wlan0 (hostapd, 192.168.50.1/24)
                            |
                        Pi-hole FTL (DNS :53, DHCP :67)
                            |
                        eth0 (192.168.29.x, uplink to Jio router)
```

- hostapd runs directly on `wlan0` (NOT the virtual `wlan0_ap`)
- Pi-hole FTL handles DHCP (192.168.50.10-250) and DNS
- dhcpcd manages eth0 only (wlan0_ap is denied in dhcpcd.conf)
- wpa_supplicant@wlan0 must be STOPPED when hostapd uses wlan0

## Config Files

| File | Purpose |
|------|---------|
| `/etc/hostapd/hostapd.conf` | WPA2 AP config (interface=wlan0) |
| `/etc/hostapd/hostapd_open.conf` | Open AP config (no WPA, for testing) |
| `/etc/dhcpcd.conf` | Has `denyinterfaces wlan0_ap` at top |
| `/etc/wpa_supplicant/wpa_supplicant.conf` | WiFi client config (prab_jiofiber) |
| `/etc/pihole/pihole.toml` | Pi-hole config including DHCP settings |

## Credentials

- Hotspot SSID: `Prabhanshu`
- Hotspot password: `cherryrocks`
- Pi-hole DHCP range: 192.168.50.10 - 192.168.50.250
- Router (wpa_supplicant): `prab_jiofiber` / `cherryrocks`

## Services

| Service | Should be | Controls |
|---------|-----------|----------|
| `hostapd` | active | WiFi AP |
| `pihole-FTL` | active | DNS + DHCP |
| `dhcpcd` | active | eth0 DHCP client |
| `wpa_supplicant@wlan0` | **stopped** | WiFi client (conflicts with hostapd on wlan0) |
| `hotspot-setup` | enabled (oneshot) | Creates wlan0_ap (legacy, not currently used) |

## Troubleshooting Flowchart

### Can't connect at all (SSID not visible)

```bash
# Is hostapd running?
systemctl status hostapd
# If dead:
sudo systemctl start hostapd
# If it fails, check wpa_supplicant isn't holding wlan0:
sudo systemctl stop wpa_supplicant@wlan0
sudo systemctl start hostapd
```

### SSID visible but connection drops in ~4 seconds

**Check 1: DHCP server running?**
```bash
sudo pihole-FTL --config dhcp.active    # must be "true"
```
If false:
```bash
sudo pihole-FTL --config dhcp.active true
sudo systemctl restart pihole-FTL
```

**Check 2: dhcpcd competing on AP interface?**
```bash
ps aux | grep dhcpcd    # should NOT show wlan0_ap
```
If it does, add `denyinterfaces wlan0_ap` to top of `/etc/dhcpcd.conf` and restart dhcpcd.

**Check 3: DHCP leases being issued?**
```bash
cat /etc/pihole/dhcp.leases    # should have entries after phone connects
```

### SSID visible, WPA2 handshake fails (EAPOL timeout)

This is the brcmfmac firmware issue. Symptoms in `journalctl -u hostapd -f`:
```
WPA: sending 1/4 msg of 4-Way Handshake
WPA: EAPOL-Key timeout
... (repeats 4 times)
WPA: PTKSTART: Retry limit 4 reached
```

**Fix sequence (try in order, test after each):**

1. **Stop wpa_supplicant** (it fights hostapd for the radio):
   ```bash
   sudo systemctl stop wpa_supplicant@wlan0
   ```

2. **Restart hostapd**:
   ```bash
   sudo systemctl restart hostapd
   ```

3. **Reload brcmfmac driver** (resets firmware state):
   ```bash
   sudo systemctl stop hostapd
   sudo rmmod brcmfmac_wcc brcmfmac
   sleep 2
   sudo modprobe brcmfmac
   sleep 2
   # Re-assign AP IP (module reload destroys it)
   sudo ip addr add 192.168.50.1/24 dev wlan0
   sudo systemctl start hostapd
   ```

4. **Test with open AP** (confirms radio works, isolates WPA issue):
   ```bash
   sudo systemctl stop hostapd
   sudo hostapd /etc/hostapd/hostapd_open.conf
   # Phone should connect to "Prabhanshu-Test" instantly
   ```

5. **Run hostapd with max debug** (capture exact failure point):
   ```bash
   sudo systemctl stop hostapd
   sudo hostapd -dd /etc/hostapd/hostapd.conf
   # Watch output, phone on auto-connect
   ```

### After switching WiFi band/channel

If you changed `hw_mode` or `channel` in hostapd.conf:

```bash
# Reload driver to reset firmware channel state
sudo systemctl stop hostapd
sudo rmmod brcmfmac_wcc brcmfmac
sleep 2
sudo modprobe brcmfmac
sleep 2
sudo ip addr add 192.168.50.1/24 dev wlan0
sudo systemctl start hostapd
```

The brcmfmac firmware caches channel/band state. A simple hostapd restart may not be enough after band changes.

## Hardware

- WiFi chip: BCM43455 (BCM4345/6)
- Driver: brcmfmac (FullMAC — firmware handles EAPOL)
- Firmware: 7.45.265 (Aug 29 2023, Cypress)
- Package: `firmware-brcm80211` 1:20241210-1+rpt4
- Kernel: 6.12.47+rpt-rpi-2712

## Known Issues

### brcmfmac + virtual AP interface (wlan0_ap)
The virtual AP interface created by `iw dev wlan0 interface add wlan0_ap type __ap` causes EAPOL frame delivery failure. hostapd sends WPA2 handshake frame 1/4 but it never reaches the client at the radio level. **Use wlan0 directly instead.**

### wpa_supplicant fighting hostapd
wpa_supplicant@wlan0 runs as a WiFi client on wlan0. If hostapd also uses wlan0, they conflict. wpa_supplicant had 1100+ auth failures trying to connect to prab_jiofiber while hostapd was running. **Stop wpa_supplicant before starting hostapd on wlan0.**

### EAPOL intermittent on fresh module load
After reloading brcmfmac, the first few WPA2 handshake attempts may time out. The handshake succeeds after several retries. Phone auto-connect handles this — just wait.

## History

- **Jan 30, 2026**: Fixed 3 root causes (dhcpcd conflict, Pi-hole DHCP disabled, virtual AP EAPOL failure). Switched from wlan0_ap to wlan0 direct. See `hotspot-fix-jan2026.md` for full writeup.
