# Multi-Boot USB: Kali Linux + Pop!_OS

233GB USB drive with Kali Live (persistent) + Pop!_OS ISO loopback boot.

## Partition Layout

| Partition | Size | Filesystem | Label | Purpose |
|-----------|------|------------|-------|---------|
| sda1 | 5GB | iso9660 | Kali Live | Kali Live boot (read-only) |
| sda2 | 4MB | vfat | (none) | EFI partition — **patched grubx64.efi + custom grub.cfg** |
| sda3 | 228GB | ext4 | persistence | Kali persistence overlay + Pop!_OS ISO + extracted kernel/initrd |

## How It Works

### GRUB Boot Chain
1. UEFI loads `grubx64.efi` from sda2
2. This EFI binary was **binary-patched** to read `$prefix/grub.cfg` instead of its embedded memdisk config
3. `grub.cfg` on sda2 presents the 5-entry menu
4. GRUB modules (ext2, gfxterm, png, etc.) are loaded from sda1's `/boot/grub/x86_64-efi/`

### Kali Boot
Standard Kali Live boot from sda1. Persistence uses sda3 (labeled `persistence`, contains `persistence.conf` with `/ union`). The `rw/` directory on sda3 holds the overlay filesystem.

### Pop!_OS Boot
The Pop!_OS ISO (`pop-os_24.04_amd64_generic_23.iso`, 3GB) lives on sda3. The kernel and initrd are **extracted directly onto sda3** (not loaded via GRUB loopback) because GRUB runs out of memory trying to loopback-mount the 3GB ISO in EFI mode.

Files on sda3:
- `pop-os_24.04_amd64_generic_23.iso` — full ISO (3GB)
- `pop-os-vmlinuz.efi` — kernel extracted from ISO's `/casper/vmlinuz.efi` (16MB)
- `pop-os-initrd.gz` — initrd extracted from ISO's `/casper/initrd.gz` (129MB)

The `iso-scan/filename=` kernel parameter tells the booted Linux to find and mount the ISO for the squashfs root filesystem.

## Key Lessons

### GRUB Loopback OOM (Feb 2026)
GRUB's `loopback` command + loading a 129MB initrd from inside a 3GB ISO causes "out of memory" in UEFI mode. Fix: extract kernel/initrd to the partition and load directly.

### Kali's grubx64.efi Memdisk (Feb 2026)
Kali's `grubx64.efi` on sda1 has a hardcoded `normal (memdisk)/grub.cfg` that ignores any on-disk config. The sda2 copy was binary-patched to `normal $prefix/grub.cfg`. This took 6 iterations to get right — do not replace sda2's `grubx64.efi`.

### Persistence Partition
`casper-rw` (4GB file on sda3) provides Pop!_OS persistence. Kali uses the ext4 partition directly via `/ union`.

## Files in This Directory

- `grub.cfg` — the custom GRUB config from sda2 (source of truth)
- `README.md` — this file

## Rebuilding

If the USB needs to be recreated:

1. Flash Kali Live ISO to USB (dd or Rufus)
2. Create sda3 ext4 partition labeled `persistence`, write `/ union` to `persistence.conf`
3. Copy Pop!_OS ISO to sda3
4. Extract kernel/initrd: `mount -o loop pop-os*.iso /mnt && cp /mnt/casper/vmlinuz.efi pop-os-vmlinuz.efi && cp /mnt/casper/initrd.gz pop-os-initrd.gz`
5. On sda2: replace `grubx64.efi` with the patched version, copy `grub.cfg`
6. The patched `grubx64.efi` is on sda2 of the current USB — back it up before reformatting

## Related Sessions
- Primary build (9.5hrs): `claude --resume 9506187e-7c4e-44e9-80bd-7ac2c4da39d2`
- OOM fix: `claude --resume` (current session, Feb 22 2026)
