# RPi5 Reachability Journey: What Was Built, What Broke, and Why

> **Date:** 2026-02-16
> **Audience:** Future self / Claude sessions working on the Pi
> **Status:** Diagnosis complete. Fixes require a dedicated session.

## Executive Summary

Over 6 sessions spanning Jan 23-31, 2026 (~20+ hours of work), an ambitious RPi5 setup was built: WiFi hotspot, Pi-hole DNS, NAS with dual drives, uplink auto-switching, and a full Rust client-server architecture (pibox). The guiding philosophy was **"The Pi is a server. It DOESN'T GO DOWN, no matter what."**

Three weeks later (Feb 16), the Pi survives only because Ethernet happens to be plugged in. WiFi is dead, the hotspot has crash-looped **126,223 times** due to a one-line config bug, one NAS drive is unmounted, Tailscale was never installed, and `rfkill` was somehow uninstalled. Remove the Ethernet cable and the Pi is a brick.

**Root cause:** `hostapd.conf` targets `interface=wlan0` but the setup script creates `wlan0_ap` as the virtual AP interface. hostapd fights `wpa_supplicant` for control of `wlan0`, loses every 2 seconds, and restarts forever.

---

## The "Always Reachable" Philosophy

From the Jan 24 session (5f2e5c43), the user's words:

> "The Pi won't 'come and go', actually the CLIENTS will come and go. Pi is a server, right? So it should act like a server, be a machine that DOESN'T GO DOWN, no matter what."

This philosophy drove every design decision:
- **Thin server, heavy clients** — Pi runs minimal services, clients do the heavy lifting
- **No stale mounts** — WebSocket protocol instead of CIFS, to avoid kernel-level hangs
- **Reproducible from scratch** — Bootstrap scripts in git, flash and rebuild from SD card
- **Uplink auto-switching** — Ethernet preferred, WiFi fallback, hotspot always on
- **Ship Computer fallback** — Telegram alerts if desktop goes offline (never deployed)

The irony: the system designed for maximum resilience has 4 of 6 reachability layers broken.

---

## Chronological History

### Jan 23 evening — Initial Recovery (session `eec651a8`, ~2h 46m)

- Pi had been sitting unused, needed fresh setup
- Connected via Ethernet, configured WiFi (wpa_supplicant + dhcpcd)
- Key question explored: "Can the Pi be a WiFi hotspot AND client simultaneously?"
- Answer: Yes, using virtual AP interface (`wlan0_ap`) alongside managed `wlan0`
- Repo: `utilities/rpi5-recovery` for initial config scripts

### Jan 23 night — Hotspot + Pi-hole (session `fe9d87b2`, ~1h 54m)

- Deployed hostapd for "Prabhanshu" hotspot network
- Created `setup-hotspot.sh` to manage `wlan0_ap` virtual interface
- Installed Pi-hole v6 for DNS blocking on the hotspot subnet
- Created `uplink-monitor.sh` for Ethernet/WiFi auto-switching
- Set up `hotspot-setup.service` to run before hostapd at boot
- **Everything worked** — hotspot broadcasting, clients could connect

### Jan 24 early AM — NAS Setup (session `748ff718`, ~1h 48m)

- Mounted Samsung T7 (exFAT) and WD Elements (NTFS) via fstab
- Installed Samba for network file sharing
- Installed Filebrowser for web-based file access
- Created mount points at `/mnt/nas/t7` and `/mnt/nas/elements`
- Added `nofail` to fstab entries (Pi boots even if drives disconnected)

### Jan 24 early AM — NAS Testing (session `d0cf2ce2`, ~1h 48m)

- Tested Samba and Filebrowser access from desktop
- Created `self-destruct.sh` and bootstrap scripts for reproducibility
- Created `/mnt/nas/prabhanshu-files` and `/mnt/nas/android` directories
- Verified everything worked end-to-end

### Jan 24 afternoon — The Crash (session `5f2e5c43`, ~13h 28m)

- **Pi went down after ~11 hours of uptime**
- Root cause: CIFS (Samba) stale mount caused kernel-level hang
- Desktop couldn't unmount, SSH hung, required hard reboot
- This is what triggered the "always reachable" philosophy
- Redesigned approach: WebSocket instead of CIFS
- Began designing the pibox Rust architecture

### Jan 24 afternoon — pibox Built (session `bb9fb260`, ~14h 18m)

- Full Rust workspace created in `embedded-device-bootstrapping/`
- `pibox-core`: Shared library (protocol, JWT auth, state machine, config)
- `pibox-server`: WebSocket server for the Pi
- `pibox-tui`: Terminal client with vim keybindings
- `pibox-gui`: Iced-based graphical client
- Code compiles but was **never deployed** to the Pi
- The session focused on architecture and implementation, not deployment

### Jan 31 — Ship Computer Concept (session `e154224a`, ~1h 1m)

- Designed "Ship Computer" fallback monitoring agent
- RPi5 pings desktop via Tailscale every 2 minutes
- If desktop offline, sends Telegram alert
- Created template scripts in `deploy/templates/`
- **Never deployed** — requires Tailscale on Pi first

---

## Current Pi State (Live Diagnosis, Feb 16 2026)

```
Host:     rpi5 (Raspberry Pi 5)
IP:       192.168.29.10 (Ethernet)
Uptime:   5 days, 11 hours
Memory:   588Mi used / 7.9Gi total
SD Card:  6.0G used / 59G total (10% used)
```

### What's Working

| Component | Status | Details |
|-----------|--------|---------|
| eth0 | UP | 192.168.29.10/24 — **only reason Pi is reachable** |
| dhcpcd | Running | DHCP client for both eth0 and wlan0 |
| Pi-hole FTL | Running | DNS blocking on port 53 (IPv4+IPv6) |
| Filebrowser | Running | Web file manager (but see NAS section) |
| Samba (smbd) | Running | File sharing (but see NAS section) |
| uplink-monitor | Running | Checks eth0/wlan0 every 10s, currently locked to Ethernet |
| mDNS | Working | `rpi5.local` resolves correctly |
| Samsung T7 | Mounted | `/mnt/nas/t7` — 922G/932G used (99% full!) |

### What's Broken

| Component | Status | Details |
|-----------|--------|---------|
| wlan0 | DOWN | WiFi client not connected to home router |
| wlan0_ap | DOWN | Virtual AP interface exists but not active |
| hostapd | CRASH LOOP | **126,223 restarts** in 5 days, every ~2 seconds |
| Hotspot "Prabhanshu" | Dead | Not broadcasting (hostapd can't start) |
| WD Elements | Not mounted | `/mnt/nas/elements` empty — drive not plugged in or UUID mismatch |
| Tailscale | Not installed | `which tailscale` → not found |
| rfkill | Not installed | `which rfkill` → not found (uninstalled somehow) |
| wpa_supplicant | Running but idle | Service active, but wlan0 has no IP — not connected to any network |

### NAS Drive Status

```
/dev/sdb1  →  /mnt/nas/t7        932G  922G used (99% FULL!)  exFAT
(missing)  →  /mnt/nas/elements   Not mounted                  NTFS
```

fstab entries exist for both drives with `nofail`, so the Pi boots regardless. But:
- T7 is nearly full — only 10G free
- Elements is physically disconnected (or UUID changed)
- Filebrowser and Samba are running but only serving T7 content

---

## Root Cause Analysis: The WiFi/Hostapd Crash Loop

### The Bug: Config Mismatch

**`/usr/local/bin/setup-hotspot.sh`** creates the virtual AP interface correctly:
```bash
iw dev wlan0 interface add wlan0_ap type __ap
ip addr add 192.168.50.1/24 dev wlan0_ap
ip link set wlan0_ap up
```

**`/etc/hostapd/hostapd.conf`** targets the WRONG interface:
```
interface=wlan0          ← BUG: should be wlan0_ap
driver=nl80211
ssid=Prabhanshu
```

### What Happens at Boot

1. `hotspot-setup.service` runs → creates `wlan0_ap` with IP 192.168.50.1 ✓
2. `wpa_supplicant` starts → claims `wlan0` in managed/station mode ✓
3. `hostapd` starts → tries to flip `wlan0` into AP mode ✗
4. Kernel: `brcmf_cfg80211_change_iface: iface validation failed: err=-16` (EBUSY)
5. hostapd exits with status 1
6. systemd restarts hostapd (default `Restart=on-failure`)
7. Goto step 3. Repeat every ~2 seconds. Forever.

### Why It Worked Initially (Jan 24)

The setup was tested and working on Jan 24. Possible explanations for why it broke:

1. **Boot ordering race condition**: On Jan 24, hostapd may have started before wpa_supplicant, winning the race for `wlan0`. After a reboot (when user left), the ordering flipped.
2. **Manual testing artifact**: During the session, services may have been started manually in the right order, masking the config bug.
3. **The config was always wrong** but happened to work because wlan0 wasn't claimed by wpa_supplicant yet during initial testing.

### The Fix (One Line)

Change `/etc/hostapd/hostapd.conf`:
```diff
-interface=wlan0
+interface=wlan0_ap
```

This makes hostapd use the virtual AP interface that `setup-hotspot.sh` already creates, avoiding the conflict with wpa_supplicant on `wlan0`.

### Secondary Concern: WiFi Client Not Connected

Even fixing hostapd won't restore WiFi client connectivity. `wlan0` is DOWN with no IP. Possible causes:
- The hostapd crash loop may have destabilized the WiFi hardware
- `rfkill` is missing — can't check if WiFi is soft-blocked
- `wpa_supplicant` is running but may not be actively trying to connect
- After 126K failed interface mode changes, the brcmfmac driver may need a clean reboot

---

## Resilience Layer Analysis

The system was designed with multiple layers of reachability. Here's what actually works:

```
Layer 5: Tailscale VPN mesh           — NOT INSTALLED     ✗
Layer 4: Hotspot "Prabhanshu"         — BROKEN (126K loop) ✗
Layer 3: WiFi client (home router)    — DOWN (wlan0 dead)  ✗
Layer 2: Ethernet (router LAN)        — WORKING            ✓  ← only lifeline
Layer 1: mDNS (rpi5.local)            — WORKING            ✓
Layer 0: Physical access (HDMI+KB)    — Always available    ✓
```

**Only layers 0-2 work.** Remove the Ethernet cable and layers 0-1 are useless without a monitor.

### What Each Layer Was Supposed to Provide

| Layer | Access From | When Needed |
|-------|-------------|-------------|
| 5 (Tailscale) | Anywhere in the world | Away from home, VPN mesh |
| 4 (Hotspot) | Within WiFi range of Pi | Router down, direct phone→Pi |
| 3 (WiFi client) | LAN via router | Normal home use, cable-free |
| 2 (Ethernet) | LAN via cable | Reliable fallback |
| 1 (mDNS) | LAN devices | Discovery without knowing IP |
| 0 (Physical) | Standing next to Pi | Last resort |

---

## Inventory: What Was Built vs. What Works

### Deployed on Pi

| Component | Location | Status | Notes |
|-----------|----------|--------|-------|
| WiFi config | `/etc/wpa_supplicant/wpa_supplicant.conf` | Config OK, wlan0 DOWN | Credentials for `prab_jiofiber` present |
| dhcpcd config | `/etc/dhcpcd.conf` | Working | Manages eth0+wlan0, denies wlan0_ap |
| Hotspot setup | `/usr/local/bin/setup-hotspot.sh` | Runs OK | Creates wlan0_ap correctly |
| hostapd config | `/etc/hostapd/hostapd.conf` | **BUGGY** | Wrong interface (`wlan0` instead of `wlan0_ap`) |
| hostapd service | systemd enabled | Crash-looping | 126,223 restarts |
| uplink-monitor | `/usr/local/bin/uplink-monitor.sh` | Running | Stuck on Ethernet (no WiFi to switch to) |
| Pi-hole v6 | FTL on port 53 | Working | DNS blocking active |
| Samba | smbd service | Running | Serving T7 only (Elements unmounted) |
| Filebrowser | filebrowser service | Running | Serving T7 only |
| NAS mounts | fstab entries | Partial | T7 mounted (99% full), Elements missing |

### In Git Repos (Never Deployed)

| Component | Repo/Path | Purpose |
|-----------|-----------|---------|
| pibox-core | `embedded-device-bootstrapping/pibox-core/` | Shared Rust library |
| pibox-server | `embedded-device-bootstrapping/pibox-server/` | WebSocket server for Pi |
| pibox-tui | `embedded-device-bootstrapping/pibox-tui/` | Terminal client |
| pibox-gui | `embedded-device-bootstrapping/pibox-gui/` | Graphical client (Iced) |
| Ship Computer | `embedded-device-bootstrapping/deploy/templates/` | Telegram fallback monitor |
| Tailscale setup | `embedded-device-bootstrapping/bootstrap/common/setup-tailscale.sh` | Install script (never run on Pi) |
| Bootstrap scripts | `pi-nas/scripts/` | Self-destruct/rebuild scripts |
| rpi5-recovery | `utilities/rpi5-recovery/` | Initial WiFi/network setup |

---

## Action Items (For a Future Session)

### Priority 1: Stop the Crash Loop and Fix WiFi

**Goal:** Stop hostapd from restarting 60,000+ times per day and restore WiFi.

Steps:
1. SSH to Pi (reachable now via Ethernet at 192.168.29.10)
2. Fix hostapd config: `interface=wlan0` → `interface=wlan0_ap`
3. Reinstall rfkill: `sudo apt install rfkill`
4. Reboot Pi cleanly (clears 126K restart counter, resets brcmfmac driver)
5. After reboot, verify:
   - `wlan0` is UP with an IP from the home router
   - `wlan0_ap` is UP at 192.168.50.1
   - hostapd is active (not crash-looping)
   - Hotspot "Prabhanshu" is visible on phone
6. If concurrent AP+STA still fails on brcmfmac, consider:
   - Dropping hotspot entirely (Tailscale makes it less critical)
   - Using a USB WiFi dongle for one of the roles

**Note:** All of these are remote-fixable via SSH. No physical access needed.

### Priority 2: Install Tailscale

**Goal:** Permanent remote access regardless of IP changes or WiFi state.

Steps:
1. Run `setup-tailscale.sh` from the repo (already written, tested on other machines)
2. Authenticate via the URL Tailscale prints
3. Verify Pi appears in tailnet: `tailscale status`
4. Test SSH via Tailscale IP from desktop
5. Add Pi's Tailscale IP to SSH config as `rpi5` alias

**The previous Forgejo plan said this required physical access — it doesn't.** The Pi is reachable on LAN right now.

### Priority 3: Fix NAS Drives

**Goal:** Both drives mounted and accessible.

Steps:
1. Check if WD Elements is physically plugged in (may need user to check)
2. If plugged in, verify UUID matches fstab: `blkid | grep sd`
3. If UUID changed, update fstab entry
4. Address T7 being 99% full — review what can be cleaned up or archived
5. Test Filebrowser and Samba with both drives mounted

### Priority 4: Deploy pibox (Optional)

**Goal:** Replace fragile CIFS mounts with WebSocket-based file access.

Steps:
1. Cross-compile pibox-server for aarch64 (Pi 5)
2. Deploy binary to Pi
3. Create systemd service
4. Test from desktop with pibox-tui

### Priority 5: Then Forgejo CI/CD

**Goal:** Self-hosted git + CI/CD on the Pi.

Only after layers 2-5 are working. Use the existing Forgejo plan as the blueprint, but verify resource assumptions (the previous plan's numbers were wrong about available RAM and disk).

### Priority 6: Ship Computer Fallback

**Goal:** Telegram alerts when desktop goes offline.

Requires Tailscale on Pi (Priority 2) and a Telegram bot token. Templates already exist in `deploy/templates/`.

---

## Lessons Learned

1. **Test after reboot, not just after manual setup.** The hostapd config worked during interactive testing but failed on reboot due to service ordering.

2. **Config files must match their setup scripts.** `setup-hotspot.sh` creates `wlan0_ap`, but `hostapd.conf` references `wlan0`. This disconnect survived 6 sessions of work.

3. **Crash loops have cascading effects.** 126K restarts in 5 days means ~350 restarts/hour, ~6/minute. Each attempt triggers kernel driver calls (`brcmf_cfg80211_change_iface`), which may have destabilized the WiFi subsystem entirely — explaining why even the WiFi client (wlan0) is down.

4. **`nofail` in fstab is essential but masks problems.** The Pi boots fine without drives, but NAS services silently serve empty mount points.

5. **Tailscale should be installed FIRST, before anything else.** It's the most reliable remote access layer and has no dependencies on WiFi or hostapd working correctly. Future Pi setups should install Tailscale immediately after first SSH connection.

6. **Check drive capacity.** T7 at 99% full will cause write failures for any new NAS operations.

---

## Quick Reference: How to Connect to the Pi Today

```bash
# Via Ethernet (current only working method)
ssh pi@192.168.29.10
ssh pi@rpi5.local

# After Priority 1 fix (WiFi restored)
# Same IPs should work, plus WiFi-assigned IP

# After Priority 2 (Tailscale installed)
ssh pi@<tailscale-ip>
# Then add to ~/.ssh/config as 'rpi5'
```
