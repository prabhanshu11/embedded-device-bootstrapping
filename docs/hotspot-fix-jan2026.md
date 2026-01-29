# RPi5 Hotspot Fix - January 30, 2026

## Problem

Clients (phone, laptop) could see "Prabhanshu" WiFi and associate, but disconnected after ~4 seconds. No IP was ever assigned.

## Root Causes Found

### 1. dhcpcd running as DHCP client on hotspot interface

`dhcpcd` was running on `wlan0_ap` (the AP interface), conflicting with Pi-hole FTL's DHCP server. dhcpcd is a DHCP **client** — it should never run on the AP interface where a DHCP **server** is needed.

Evidence:
- `dhcpcd: [BPF BOOTP] wlan0_ap` process running
- `dhcpcd: [BPF ARP] wlan0_ap 169.254.64.221` (link-local fallback)
- `dhcp.leases` was 0 bytes

**Fix:** Added `denyinterfaces wlan0_ap` at top of `/etc/dhcpcd.conf`, then `sudo systemctl restart dhcpcd`.

### 2. Pi-hole FTL DHCP was disabled

`pihole-FTL --config dhcp.active` returned `false`. No DHCP server was running for the hotspot network, so clients could never get an IP.

**Fix:** Enabled Pi-hole DHCP for the hotspot subnet:
```bash
sudo pihole-FTL --config dhcp.start 192.168.50.10
sudo pihole-FTL --config dhcp.end 192.168.50.250
sudo pihole-FTL --config dhcp.router 192.168.50.1
sudo pihole-FTL --config dhcp.netmask 255.255.255.0
sudo pihole-FTL --config dhcp.leaseTime 24h
sudo pihole-FTL --config dhcp.active true
sudo systemctl restart pihole-FTL
```

### 3. WPA2 EAPOL 4-way handshake failure (UNRESOLVED)

After fixing DHCP, clients still couldn't connect. Verbose hostapd logging (`logger_syslog_level=0`) revealed the WPA2 4-way handshake was timing out:

```
wlan0: STA 3a:0e:cd:71:d0:8f WPA: sending 1/4 msg of 4-Way Handshake
wlan0: STA 3a:0e:cd:71:d0:8f WPA: EAPOL-Key timeout
wlan0: STA 3a:0e:cd:71:d0:8f WPA: sending 1/4 msg of 4-Way Handshake
wlan0: STA 3a:0e:cd:71:d0:8f WPA: EAPOL-Key timeout
... (4 retries)
wlan0: STA 3a:0e:cd:71:d0:8f WPA: PTKSTART: Retry limit 4 reached
wlan0: STA 3a:0e:cd:71:d0:8f MLME: MLME-DEAUTHENTICATE.indication (reason 15)
```

hostapd sends EAPOL message 1/4 but the client never responds with message 2/4. The frames aren't being delivered at the radio/firmware level.

**Things tried (all failed):**
- Stopped `wpa_supplicant@wlan0` (was potentially intercepting EAPOL frames)
- Stopped D-Bus `wpa_supplicant` service
- Brought `wlan0` down while using `wlan0_ap` virtual interface
- Switched hostapd from `wlan0_ap` (virtual) to `wlan0` directly
- Enabled `wmm_enabled=1` (some phones require WMM for WPA2)
- Reloaded brcmfmac kernel module (`rmmod brcmfmac_wcc brcmfmac && modprobe brcmfmac`)

**Open network works:** Removing all WPA config from hostapd allows clients to connect immediately, proving the radio hardware is functional. The issue is specifically EAPOL frame delivery in AP mode.

**Firmware:** BCM4345/6 (brcmfmac), version 7.45.265 (Aug 29 2023), Cypress.

## Current State

- **hostapd:** Running on `wlan0` directly (not virtual `wlan0_ap`)
- **SSID:** Prabhanshu-Test (OPEN, no WPA2)
- **DHCP:** Pi-hole FTL, range 192.168.50.10-250, gateway 192.168.50.1
- **dhcpcd.conf:** Has `denyinterfaces wlan0_ap` (still useful if reverting to virtual interface)
- **Pi-hole interface:** `dns.interface = wlan0`
- **wpa_supplicant:** Stopped (was interfering, not needed since eth0 is uplink)

## Architecture Change: wlan0_ap -> wlan0

The bootstrap scripts originally used a virtual AP interface (`wlan0_ap`) created from `wlan0`. This allowed simultaneous AP + STA modes. Since eth0 is the primary uplink, `wlan0` is used directly for AP mode now.

| Component | Old (wlan0_ap) | New (wlan0) |
|-----------|---------------|-------------|
| hostapd | `interface=wlan0_ap` | `interface=wlan0` |
| Pi-hole FTL | `dns.interface=wlan0_ap` | `dns.interface=wlan0` |
| dhcpcd | Needed `denyinterfaces wlan0_ap` | N/A (wlan0 not a DHCP client) |
| hotspot-setup.service | Creates wlan0_ap, assigns 192.168.50.1/24 | Not needed, IP on wlan0 |
| Fallback WiFi uplink | wlan0 available via wpa_supplicant | NOT available (wlan0 is AP) |

**Trade-off:** No WiFi fallback uplink if eth0 goes down. Acceptable since the Pi is wired to the router.

## TODO

- [ ] Fix WPA2 EAPOL handshake (firmware update? different hostapd version? external USB WiFi adapter?)
- [ ] Update `setup-hotspot.sh` to use `wlan0` instead of `wlan0_ap`
- [ ] Update `hostapd.conf.template` to use `wlan0`
- [ ] Remove dependency on standalone dnsmasq (Pi-hole FTL handles DHCP)
- [ ] Consider: Update hotspot-setup.service to not create wlan0_ap
- [ ] Investigate: Does `rpi-update` provide newer brcmfmac firmware that fixes EAPOL?

## Services on Pi (reference)

| Service | Port/Interface | Purpose |
|---------|---------------|---------|
| hostapd | wlan0 | WiFi hotspot "Prabhanshu" |
| pihole-FTL | :53 (DNS), :67 (DHCP), :80/:443 (web) | Ad blocking + DHCP |
| filebrowser | :8080 | Web file manager at /mnt/nas |
| smbd/nmbd/winbind | :445 | Samba NAS shares |
| dhcpcd | eth0 only | DHCP client for ethernet uplink |

## Diagnosis Commands

```bash
# Check hostapd WPA handshake (verbose)
journalctl -u hostapd -f

# Check DHCP leases
pihole-FTL --config dhcp
cat /etc/pihole/dhcp.leases

# Check which interfaces dhcpcd manages
ps aux | grep dhcpcd

# Check Pi-hole DHCP logs
cat /var/log/pihole/FTL.log | grep -i dhcp

# Check EAPOL in dmesg
dmesg | grep -i brcm
```
