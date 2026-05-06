# Pi5 Claude Summariser Node — Setup & Recovery

Brings up a Pi5 as the **daily Obsidian vault aggregation node**. After flash/bootstrap (covered by `bootstrap/rpi5/`), run these steps to install Claude Code, join the vault sync, and enable the daily 5:30am summary cron.

Companion docs:
- Architecture / decisions: `~/Programs/obsidian-sync-aggregation/docs/pi5-aggregation.md`
- Bootstrap entry point for whole project: `~/Programs/obsidian-sync-aggregation/CONTINUATION.md`

## Recovery scenario

Use this doc when:
- Pi5 SD card was wiped or replaced
- Migrating to a new Pi
- Setting up a second summariser node (redundancy)

## Prerequisites

- Pi5 has been flashed and base-bootstrapped per `bootstrap/rpi5/setup-hotspot.sh`
- SSH access works: `ssh pi@rpi5` (Tailscale MagicDNS) or `ssh pi@<lan-ip>`
- Pi5 is on the same network as at least one other vault peer (for initial Syncthing sync)
- Claude Pro/Max plan account credentials available

## Step 1: Install Claude Code

```bash
ssh pi@rpi5
curl -fsSL https://claude.ai/install.sh | bash
```

Auto-detects ARM64. Verify:
```bash
claude --version
which claude  # expect ~/.local/bin/claude or /usr/local/bin/claude
```

## Step 2: One-time login

```bash
claude
```

Follow the browser auth flow. The Pi has no GUI — copy the URL printed to terminal, paste into a browser on a paired device, complete login. Token is stored locally on Pi5 in `~/.config/claude/`.

**Backup the auth token after first login** (for fast recovery later):
```bash
# After login, copy token to encrypted backup
tar czf - ~/.config/claude | pass insert -m claude/pi5-auth-token-backup
```

To restore on a fresh Pi5:
```bash
pass show claude/pi5-auth-token-backup | tar xzf - -C ~/
```

(Ensures you don't have to re-do browser auth after every wipe.)

## Step 3: Join the vault Syncthing

```bash
sudo apt install syncthing
systemctl --user enable --now syncthing
```

Then via web UI (SSH-tunnel from your laptop):
```bash
# from your workstation
ssh -L 8385:127.0.0.1:8384 pi@rpi5
# now open http://127.0.0.1:8385 locally
```

In the Pi5 syncthing UI:
1. Get the Pi5's device ID (top-right `Actions → Show ID`)
2. On laptop's syncthing UI, add Pi5 as a device using that ID
3. Share the `obsidian-vault` folder with Pi5
4. Accept on Pi5 side, set local path to `~/obsidian-vault`
5. Wait for initial sync (~12 MB, fast)

## Step 4: Aggregation script

```bash
mkdir -p ~/bin
cat > ~/bin/aggregate-vault.sh <<'EOF'
#!/bin/bash
set -euo pipefail

VAULT="$HOME/obsidian-vault"
YESTERDAY=$(date -d 'yesterday' +%F)
WEEK=$(date +%G-W%V)
MONTH=$(date +%Y-%m)

mkdir -p "$VAULT/Weekly" "$VAULT/Monthly"

WEEKLY="$VAULT/Weekly/${WEEK}.md"
MONTHLY="$VAULT/Monthly/${MONTH}.md"
DAILY="$VAULT/00 - Daily/${YESTERDAY}.md"

if [ ! -f "$DAILY" ]; then
  echo "$(date -Iseconds): no daily note for $YESTERDAY — skipping"
  exit 0
fi

[ -f "$WEEKLY" ] || echo "# Week $WEEK" > "$WEEKLY"
[ -f "$MONTHLY" ] || echo "# Month $MONTH" > "$MONTHLY"

cd "$VAULT"
claude -p "Read '00 - Daily/${YESTERDAY}.md'. If a section dated '## $YESTERDAY' already exists in 'Weekly/${WEEK}.md' or 'Monthly/${MONTH}.md', do nothing. Otherwise append a 3-bullet summary under '## $YESTERDAY' to BOTH files. Do not modify the daily note."
EOF
chmod +x ~/bin/aggregate-vault.sh
```

## Step 5: Schedule daily 5:30am

Use systemd user timer (more robust than cron, has `Persistent=true` for missed runs):

```bash
mkdir -p ~/.config/systemd/user
cat > ~/.config/systemd/user/aggregate-vault.service <<'EOF'
[Unit]
Description=Daily Obsidian vault aggregation

[Service]
Type=oneshot
ExecStart=%h/bin/aggregate-vault.sh
StandardOutput=append:%h/aggregate-vault.log
StandardError=append:%h/aggregate-vault.log
EOF

cat > ~/.config/systemd/user/aggregate-vault.timer <<'EOF'
[Unit]
Description=Trigger vault aggregation daily at 5:30am

[Timer]
OnCalendar=*-*-* 05:30:00
Persistent=true

[Install]
WantedBy=timers.target
EOF

systemctl --user daemon-reload
systemctl --user enable --now aggregate-vault.timer
loginctl enable-linger pi   # ensures user services run without an active login session
```

Verify:
```bash
systemctl --user list-timers aggregate-vault.timer
journalctl --user -u aggregate-vault.service -f  # follow logs
```

## Step 6: Verify end-to-end

Manual run:
```bash
~/bin/aggregate-vault.sh
```

Check:
- `~/obsidian-vault/Weekly/<this-week>.md` was created/updated
- `~/obsidian-vault/Monthly/<this-month>.md` was created/updated
- Syncthing pushed those changes to other peers (check on phone or desktop)

## Recovery checklist (after wipe)

1. ✅ Flash + base bootstrap — `flash/flash-sd.sh` + `bootstrap/rpi5/setup-hotspot.sh`
2. ✅ Restore SSH keys per `bootstrap/common/base-setup.sh`
3. ✅ Install Claude Code (Step 1 above)
4. ✅ Restore Claude auth from `pass` (Step 2 backup) — saves the browser flow
5. ✅ Install + join Syncthing (Step 3) — wait for vault to download
6. ✅ Recreate aggregation script + systemd timer (Steps 4–5)
7. ✅ Verify (Step 6)

## Where this lives in the repo ecosystem

| Repo | What it owns |
|------|--------------|
| `embedded-device-bootstrapping` (this repo) | **Pi5 setup commands** — installation steps, recovery checklist (this doc) |
| `obsidian-sync-aggregation` | **Why and design** — architecture decisions, bug findings, alternatives considered |
| `local-bootstrapping` | **Workstation bootstrap** — installing Obsidian on desktop/laptop, joining vault |
| `pi-nas` | **NAS-specific setup** — Samba, Filebrowser, USB drive mounts |

## Open issues to handle on first deploy

- **Token cost decision** — default to Haiku for daily summaries (cheap), Sonnet for weekly/monthly rollups if you want them deeper. Can be set via `claude --model haiku-4.5` flag in the script.
- **Vault path** — script assumes `~/obsidian-vault`. Adjust if you mount it elsewhere.
- **Idempotency of `claude -p`** — the prompt asks Claude to skip if today's section already exists. Test this before relying on it; alternative is shell-side `grep -q "^## $YESTERDAY$"` guard.
