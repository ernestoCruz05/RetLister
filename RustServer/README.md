# RetLister Server

The central backend for the RetLister ecosystem. It is a high-performance REST API written in Rust using Axum and SQLite.

## Features

* **Core API:** CRUD operations for inventory management.
* **Optimization Engine:** Spawns a Python sidecar process to calculate 3D loading plans and 2D cutting optimizations.
* **Concurrency:** SQLite configured in WAL mode to handle concurrent requests from the Proxy and Tauri clients.
* **Robust Logging:** Tracing subscriber integration for request debugging.

## Requirements

* **Rust:** Stable toolchain (1.70+).
* **Python:** 3.10+ (Required for `optimizer.py`).
    * Ensure `python3` or `py` (Windows) is in your PATH.

## Configuration

The server runs on `0.0.0.0:8000` by default.
Database file is created automatically at `data/retlister.db`.

## Running

1.  Install dependencies and build:
    ```powershell
    cargo build --release
    ```

2.  Run the server:
    ```powershell
    cargo run --release
    ```

3.  (Optional) Populate with dummy data:
    ```powershell
    .\populate_db.ps1
    ```

## API Endpoints

### Inventory
* `GET /list` - Retrieve all items.
* `POST /add` - Create a new item.
* `POST /update/:id` - Edit item details.
* `DELETE /remove/:id` - Delete a specific item.
* `POST /delete_batch` - Bulk deletion.

### Logic & Search
* `GET /search` - Filter by dimensions and material.
* `POST /optimize` - Calculates 3D van loading plans (delegates to Python).
* `POST /optimize_cuts` - Calculates 2D cutting layouts.

### System
* `GET /health` - Liveness probe.
* `GET /stats` - Aggregated material statistics.