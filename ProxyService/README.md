# RetLister Proxy Service

Proxy service for Windows 11 that bridges Windows XP client to main RetLister server via Ethernet.

## Architecture

```
[WinXP Client] --- Ethernet --- [Win11 Proxy:8001] --- localhost --- [Main Server:8000]
```

## Features

- Transparent request forwarding to main server
- Local SQLite cache for offline operation
- Sync queue for changes made while offline
- Health monitoring endpoints
- Automatic fallback on main server unavailability

## Endpoints

### Health & Status
- `GET /health` - Proxy and main server status
- `GET /sync/status` - Pending sync operations

### Resto Operations
- `GET /list` - List all restos
- `POST /add` - Add new resto
- `DELETE /remove/:id` - Remove resto by ID
- `POST /search` - Search for suitable resto

All resto endpoints are proxied to main server with local fallback.

## Running

```powershell
cd ProxyService
cargo run --release
```

Listens on `0.0.0.0:8001` (accessible from network).

## Database

Location: `data/proxy.db`

Tables:
- `restos` - Local mirror of main database
- `sync_queue` - Offline operations awaiting sync
- `sync_metadata` - Last sync timestamp

## Configuration

Edit `src/main.rs`:
- Main server URL (default: `http://localhost:8000`)
- Listen address (default: `0.0.0.0:8001`)
