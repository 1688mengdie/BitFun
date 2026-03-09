#!/usr/bin/env bash
# BitFun Relay Server — remote deploy script.
#
# Syncs the relay-server source to a remote server, rebuilds the Docker image
# and restarts the container. All three deployment scenarios (public relay,
# LAN relay, NAT traversal relay) use the same code and the same config.
#
# Usage:
#   bash remote-deploy.sh <server> [options]
#
# Example:
#   bash remote-deploy.sh 116.204.120.240
#   bash remote-deploy.sh relay.example.com --first
#
# Prerequisites:
#   - SSH access to the server (key-based auth configured in ~/.ssh/config)
#   - Docker + Docker Compose installed on the server

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

REMOTE_DIR="/opt/bitfun-relay"
SSH_OPTS="-o ConnectTimeout=10 -o StrictHostKeyChecking=accept-new"

SKIP_HEALTH_CHECK=false
FIRST_DEPLOY=false

usage() {
  cat <<'EOF'
BitFun Relay Server — remote deploy script

Usage:
  bash remote-deploy.sh <server> [options]

Arguments:
  <server>               SSH host (IP or hostname from ~/.ssh/config)

Options:
  --first                First-time deploy (creates remote dir structure)
  --skip-health-check    Skip post-deploy health check
  -h, --help             Show this help message

Examples:
  bash remote-deploy.sh 116.204.120.240 --first   # first time
  bash remote-deploy.sh 116.204.120.240            # update
EOF
}

# ── Parse arguments ──────────────────────────────────────────────

if [ $# -lt 1 ]; then
  usage
  exit 1
fi

SERVER=""
for arg in "$@"; do
  case "$arg" in
    --first) FIRST_DEPLOY=true ;;
    --skip-health-check) SKIP_HEALTH_CHECK=true ;;
    -h|--help) usage; exit 0 ;;
    -*) echo "Unknown option: $arg"; usage; exit 1 ;;
    *)
      if [ -z "$SERVER" ]; then
        SERVER="$arg"
      else
        echo "Unexpected argument: $arg"; usage; exit 1
      fi
      ;;
  esac
done

if [ -z "$SERVER" ]; then
  echo "Error: <server> argument is required."
  usage
  exit 1
fi

if [ ! -d "$SCRIPT_DIR/src" ]; then
  echo "Error: Source directory not found: $SCRIPT_DIR/src"
  exit 1
fi

echo "=== BitFun Relay Server — Remote Deploy ==="
echo "Server:  $SERVER"
echo "Remote:  $REMOTE_DIR"
if [ "$FIRST_DEPLOY" = true ]; then
  echo "Mode:    First-time deploy"
else
  echo "Mode:    Update"
fi
echo ""

# ── 1. Test SSH connectivity ─────────────────────────────────────

echo "[1/6] Testing SSH connection..."
if ! ssh $SSH_OPTS "$SERVER" "echo ok" >/dev/null 2>&1; then
  echo "Error: Cannot connect to $SERVER via SSH."
  exit 1
fi
echo "  OK."

# ── 2. Ensure remote directory ───────────────────────────────────

if [ "$FIRST_DEPLOY" = true ]; then
  echo "[2/6] Creating remote directory $REMOTE_DIR ..."
  ssh $SSH_OPTS "$SERVER" "mkdir -p $REMOTE_DIR/src $REMOTE_DIR/static"
else
  echo "[2/6] Verifying remote directory..."
  if ! ssh $SSH_OPTS "$SERVER" "test -d $REMOTE_DIR"; then
    echo "Error: $REMOTE_DIR not found. Use --first for initial deploy."
    exit 1
  fi
fi
echo "  OK."

# ── 3. Stop old container ────────────────────────────────────────

echo "[3/6] Stopping old container (if running)..."
ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose down 2>/dev/null || true"
echo "  Done."

# ── 4. Sync files ────────────────────────────────────────────────

echo "[4/6] Syncing files..."

echo "  src/ ..."
rsync -az --delete \
  -e "ssh $SSH_OPTS" \
  "$SCRIPT_DIR/src/" \
  "$SERVER:$REMOTE_DIR/src/"

echo "  Cargo.toml, Dockerfile, docker-compose.yml ..."
scp -q $SSH_OPTS \
  "$SCRIPT_DIR/Cargo.toml" \
  "$SCRIPT_DIR/Dockerfile" \
  "$SCRIPT_DIR/docker-compose.yml" \
  "$SERVER:$REMOTE_DIR/"

if [ "$FIRST_DEPLOY" = true ]; then
  echo "  static/ ..."
  rsync -az \
    -e "ssh $SSH_OPTS" \
    "$SCRIPT_DIR/static/" \
    "$SERVER:$REMOTE_DIR/static/"
fi

echo "  Done."

# ── 5. Build ─────────────────────────────────────────────────────

echo "[5/6] Building Docker image (may take a few minutes)..."
ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose build 2>&1 | tail -5"
echo "  Build complete."

# ── 6. Start ─────────────────────────────────────────────────────

echo "[6/6] Starting container..."
ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose up -d"

# ── Health check ─────────────────────────────────────────────────

if [ "$SKIP_HEALTH_CHECK" = true ]; then
  echo ""
  echo "Health check skipped."
else
  echo ""
  echo "Waiting for service to start..."
  sleep 3
  MAX_RETRIES=6
  RETRY=0
  while [ $RETRY -lt $MAX_RETRIES ]; do
    HEALTH=$(ssh $SSH_OPTS "$SERVER" "curl -fsS --max-time 5 http://127.0.0.1:9700/health 2>/dev/null" || echo "FAIL")
    if [ "$HEALTH" != "FAIL" ]; then
      echo "Health check passed:"
      echo "  $HEALTH"
      break
    fi
    RETRY=$((RETRY + 1))
    if [ $RETRY -lt $MAX_RETRIES ]; then
      echo "  Retry $RETRY/$MAX_RETRIES in 3s..."
      sleep 3
    else
      echo "Warning: health check failed after $MAX_RETRIES attempts."
      echo "Check: ssh $SERVER 'cd $REMOTE_DIR && docker compose logs --tail=30'"
    fi
  done
fi

echo ""
echo "=== Deploy complete ==="
echo "Relay:  http://$SERVER:9700"
echo "Health: http://$SERVER:9700/health"
