#!/bin/bash
# Generic deploy script: push files to Pi and install systemd service
#
# Usage: ./deploy/deploy.sh --host pi@10.55.0.2 --files "src/app.py src/config.py" --service my-app
#
# Options:
#   --host USER@IP        SSH target (required)
#   --files "f1 f2 ..."   Files/dirs to copy (required)
#   --service NAME        systemd service name (required)
#   --dest DIR            Remote destination (default: ~/NAME)
#   --template FILE       systemd unit template (default: deploy/templates/app.service.template)
#   --exec-start CMD      ExecStart command for systemd unit (required if using template)
#   --description DESC    Service description (default: "NAME service")
#   --user USER           User to run service as (default: pi)
#   --password PASS       SSH password for sshpass (optional, prefers key auth)
#   --restart-only        Skip file copy, just restart service

set -e

# === Defaults ===
HOST=""
FILES=""
SERVICE=""
DEST=""
TEMPLATE=""
EXEC_START=""
DESCRIPTION=""
SVC_USER="pi"
PASSWORD=""
RESTART_ONLY=false

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# === Parse Arguments ===
while [[ $# -gt 0 ]]; do
    case "$1" in
        --host)         HOST="$2"; shift 2 ;;
        --files)        FILES="$2"; shift 2 ;;
        --service)      SERVICE="$2"; shift 2 ;;
        --dest)         DEST="$2"; shift 2 ;;
        --template)     TEMPLATE="$2"; shift 2 ;;
        --exec-start)   EXEC_START="$2"; shift 2 ;;
        --description)  DESCRIPTION="$2"; shift 2 ;;
        --user)         SVC_USER="$2"; shift 2 ;;
        --password)     PASSWORD="$2"; shift 2 ;;
        --restart-only) RESTART_ONLY=true; shift ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

# === Validation ===
if [[ -z "$HOST" ]] || [[ -z "$SERVICE" ]]; then
    echo "Usage: $0 --host USER@IP --files 'f1 f2' --service NAME [options]"
    echo ""
    echo "Required:"
    echo "  --host USER@IP        SSH target"
    echo "  --service NAME        systemd service name"
    echo "  --files 'f1 f2 ...'   Files to deploy (unless --restart-only)"
    echo ""
    echo "Optional:"
    echo "  --dest DIR            Remote dir (default: ~/SERVICE)"
    echo "  --template FILE       systemd unit template"
    echo "  --exec-start CMD      ExecStart for template"
    echo "  --description DESC    Service description"
    echo "  --user USER           Service user (default: pi)"
    echo "  --password PASS       SSH password (sshpass)"
    echo "  --restart-only        Just restart the service"
    exit 1
fi

if [[ "$RESTART_ONLY" == "false" ]] && [[ -z "$FILES" ]]; then
    echo "ERROR: --files is required unless using --restart-only"
    exit 1
fi

DEST="${DEST:-\$HOME/$SERVICE}"
DESCRIPTION="${DESCRIPTION:-$SERVICE service}"
TEMPLATE="${TEMPLATE:-$SCRIPT_DIR/templates/app.service.template}"

# SSH/SCP command prefix
SSH_CMD="ssh -o StrictHostKeyChecking=accept-new"
SCP_CMD="scp -o StrictHostKeyChecking=accept-new"
if [[ -n "$PASSWORD" ]]; then
    if ! command -v sshpass &>/dev/null; then
        echo "ERROR: sshpass required for password auth (install: sudo pacman -S sshpass)"
        exit 1
    fi
    SSH_CMD="sshpass -p $PASSWORD $SSH_CMD"
    SCP_CMD="sshpass -p $PASSWORD $SCP_CMD"
fi

PI_IP=$(echo "$HOST" | cut -d@ -f2)

echo "=== Deploying $SERVICE to $HOST ==="
echo ""

# === Check connectivity ===
echo "[1/5] Checking connectivity..."
if ! ping -c 1 -W 2 "$PI_IP" >/dev/null 2>&1; then
    echo "ERROR: Cannot reach $PI_IP"
    exit 1
fi
echo "  - $PI_IP is reachable"

if $RESTART_ONLY; then
    echo ""
    echo "[2-3/5] Skipping file copy (--restart-only)"
else
    # === Create remote directory ===
    echo ""
    echo "[2/5] Creating remote directory..."
    $SSH_CMD "$HOST" "mkdir -p $DEST"
    echo "  - Created $DEST"

    # === Copy files ===
    echo ""
    echo "[3/5] Copying files..."
    for f in $FILES; do
        if [[ -d "$f" ]]; then
            $SCP_CMD -r "$f" "$HOST:$DEST/"
        else
            $SCP_CMD "$f" "$HOST:$DEST/"
        fi
        echo "  - Copied $f"
    done
fi

# === Install systemd service ===
echo ""
echo "[4/5] Installing systemd service..."

if [[ -n "$EXEC_START" ]] && [[ -f "$TEMPLATE" ]]; then
    # Read template and substitute placeholders
    UNIT_CONTENT=$(cat "$TEMPLATE")
    UNIT_CONTENT="${UNIT_CONTENT//%%DESCRIPTION%%/$DESCRIPTION}"
    UNIT_CONTENT="${UNIT_CONTENT//%%USER%%/$SVC_USER}"
    UNIT_CONTENT="${UNIT_CONTENT//%%EXEC_START%%/$EXEC_START}"
    UNIT_CONTENT="${UNIT_CONTENT//%%WORKING_DIR%%/$DEST}"

    echo "$UNIT_CONTENT" | $SSH_CMD "$HOST" "sudo tee /etc/systemd/system/${SERVICE}.service > /dev/null"
    echo "  - Installed ${SERVICE}.service from template"
else
    # Check if service already exists on remote
    if $SSH_CMD "$HOST" "test -f /etc/systemd/system/${SERVICE}.service"; then
        echo "  - Using existing ${SERVICE}.service"
    else
        echo "  WARNING: No template and no existing service file"
        echo "  Create /etc/systemd/system/${SERVICE}.service manually"
    fi
fi

# === Reload and restart ===
echo ""
echo "[5/5] Restarting service..."
$SSH_CMD "$HOST" "sudo systemctl daemon-reload && sudo systemctl enable $SERVICE && sudo systemctl restart $SERVICE"
sleep 2

# === Verify ===
echo ""
echo "=== Verifying ==="
SERVICE_STATUS=$($SSH_CMD "$HOST" "systemctl is-active $SERVICE 2>/dev/null" || true)

if [[ "$SERVICE_STATUS" == "active" ]]; then
    echo "  SUCCESS: $SERVICE is running"
else
    echo "  WARNING: $SERVICE status is '$SERVICE_STATUS'"
    echo ""
    echo "  Check logs: ssh $HOST 'sudo journalctl -u $SERVICE -n 20'"
fi

echo ""
echo "=== Deploy Complete ==="
