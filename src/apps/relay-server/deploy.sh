#!/usr/bin/env bash
# BitFun Relay Server — one-click deploy script.
# Usage:  bash deploy.sh [--build-mobile]
#
# Prerequisites: Docker, Docker Compose

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

BUILD_MOBILE=false
for arg in "$@"; do
  case "$arg" in
    --build-mobile) BUILD_MOBILE=true ;;
  esac
done

echo "=== BitFun Relay Server Deploy ==="

# Build mobile web static files if requested
if [ "$BUILD_MOBILE" = true ] && [ -d "$PROJECT_ROOT/src/mobile-web" ]; then
  echo "[1/3] Building mobile web client..."
  cd "$PROJECT_ROOT/src/mobile-web"
  npm ci
  npm run build
  mkdir -p "$SCRIPT_DIR/static"
  cp -r dist/* "$SCRIPT_DIR/static/"
  cd "$SCRIPT_DIR"
  echo "  Mobile web built → $SCRIPT_DIR/static/"
else
  echo "[1/3] Skipping mobile web build (use --build-mobile to include)"
fi

# Build and start containers
echo "[2/3] Building Docker images..."
cd "$SCRIPT_DIR"
docker compose build

echo "[3/3] Starting services..."
docker compose up -d

echo ""
echo "=== Deploy complete ==="
echo "Relay server running on port 9700"
echo "Caddy proxy on ports 80/443"
echo ""
echo "Check status:  docker compose ps"
echo "View logs:     docker compose logs -f relay-server"
echo "Stop:          docker compose down"
