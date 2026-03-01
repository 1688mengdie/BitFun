# BitFun Relay Server

WebSocket relay server for BitFun Remote Connect. Provides room-based message relaying between desktop and mobile clients with E2E encryption support.

## Features

- Room-based WebSocket relay
- End-to-end encrypted message passthrough (server cannot decrypt)
- Heartbeat-based connection management
- Static file serving for mobile web client
- Docker deployment ready

## Quick Start

### Docker (Recommended)

```bash
# One-click deploy
bash deploy.sh

# With mobile web client
bash deploy.sh --build-mobile
```

### Manual

```bash
# From project root
cargo build --release -p bitfun-relay-server

# Run
RELAY_PORT=9700 ./target/release/bitfun-relay-server
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RELAY_PORT` | `9700` | Server listen port |
| `RELAY_STATIC_DIR` | `./static` | Path to mobile web static files |
| `RELAY_ROOM_TTL` | `3600` | Room TTL in seconds (0 = no expiry) |

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/api/info` | GET | Server info |
| `/ws` | WebSocket | Main relay endpoint |

## WebSocket Protocol

### Client → Server

```json
// Create a room (desktop)
{ "type": "create_room", "room_id": "...", "device_id": "...", "device_type": "desktop", "public_key": "base64..." }

// Join a room (mobile)
{ "type": "join_room", "room_id": "...", "device_id": "...", "device_type": "mobile", "public_key": "base64..." }

// Relay an encrypted message
{ "type": "relay", "room_id": "...", "encrypted_data": "base64...", "nonce": "base64..." }

// Heartbeat
{ "type": "heartbeat" }
```

### Server → Client

```json
// Peer joined notification
{ "type": "peer_joined", "device_id": "...", "device_type": "...", "public_key": "base64..." }

// Relayed message
{ "type": "relay", "from_device_id": "...", "encrypted_data": "base64...", "nonce": "base64..." }

// Peer disconnected
{ "type": "peer_disconnected", "device_id": "..." }

// Heartbeat acknowledgment
{ "type": "heartbeat_ack" }
```

## Self-Hosted Deployment

1. Clone the repository
2. Navigate to `src/apps/relay-server/`
3. Run `bash deploy.sh --build-mobile`
4. Configure DNS/firewall as needed
5. In BitFun desktop, select "Custom Server" and enter your server URL

## Architecture

```
Mobile Phone ──WSS──► Relay Server ◄──WSS── Desktop Client
                         │
                    E2E Encrypted
                    (server cannot
                     read messages)
```

The relay server only manages rooms and forwards opaque encrypted payloads. All encryption/decryption happens on the client side.
