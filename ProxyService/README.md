# RetLister Proxy Service

A specialized middleware service designed to bridge modern infrastructure with legacy Windows XP clients. It runs on the Windows 11 host and ensures reliable communication between the unstable legacy network interface and the main backend.

## Architecture

Legacy clients (Win32APP) communicate exclusively with this Proxy Service via HTTP/1.0 over Ethernet. The Proxy Service manages the connection to the main RustServer, providing:

1.  **Protocol Translation:** Bridges simple HTTP requests from the C client to the modern REST API.
2.  **Offline Capability:** Maintains a local SQLite mirror (`restos` table) of the main inventory, allowing the XP client to read data even if the main server is temporarily unreachable or if the network stutters.
3.  **Asynchronous Synchronization:** Writes from the client are acknowledged immediately, saved to a local `sync_queue`, and pushed to the main server in the background.
4.  **Cache Warming:** Automatically fetches the latest inventory from the Main Server to keep the local mirror up to date.

## Features

- **Transparent Forwarding:** Proxies requests to `localhost:8000` when available.
- **Local Fallback:** Serves read requests from `data/proxy.db` when the main server is offline.
- **Reliable Writes:** specific "INSERT" and "DELETE" operations are queued if the upstream connection fails.
- **Health Monitoring:** Exposes endpoints for the client to check upstream connectivity and sync status.

## Configuration

Configuration is currently handled in `src/main.rs`.

* **Listen Address:** `0.0.0.0:8001` (Accessible to external devices/VMs)
* **Main Server URL:** `http://localhost:8000`
* **Sync Interval:** 30 seconds

## API Endpoints

### Operations
* `GET /list` - Returns inventory (from Upstream or Cache).
* `POST /add` - Adds a new item (Queued if offline).
* `DELETE /remove/:id` - Removes an item (Queued if offline).
* `POST /search` - Search inventory (Local fallback available).

### Diagnostics
* `GET /health` - Returns the status of the Proxy and its connection to the Main Server.
* `GET /sync/status` - Returns the number of pending changes waiting to upload.

## Running

1.  Ensure the main **RustServer** is running on port 8000.
2.  Build and run the proxy:
    ```powershell
    cd ProxyService
    cargo run --release
    ```
3.  The service will listen on `0.0.0.0:8001`. Ensure your Windows Firewall allows inbound connections on this port if connecting from a physical XP machine or VM.