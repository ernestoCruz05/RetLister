# RetLister

RetLister is a distributed inventory management system designed to track wood leftovers ("restos") and optimize cutting plans. It allows modern Windows 11 workstations and legacy Windows XP machines to operate on a unified database.

The system bridges modern HTTP/JSON APIs with legacy hardware restrictions through a custom proxy architecture.

## System Architecture

The system consists of four distinct components:

1.  **RustServer (Backend):** The central authority. A Rust/Axum REST API backed by SQLite. It handles data persistence and invokes a Python sidecar for complex bin-packing optimizations.
2.  **ProxyService (Middleware):** A Rust service running on the Windows 11 host. It acts as a bridge for the Windows XP client, providing:
    * Protocol translation (bridging the XP machine via Ethernet).
    * Offline capability (local SQLite cache).
    * Synchronization queues for reliable data transfer.
3.  **TauriAPP (Modern Client):** A Windows 11 desktop application using Tauri (Rust + React). It provides a full dashboard, 3D visualizations, and inventory management.
4.  **Win32APP (Legacy Client):** A native C (Win32 API) application optimized for Windows XP. It communicates exclusively with the ProxyService via WinInet.

## Repository Structure

* `RustServer/` - Central API and Database.
* `ProxyService/` - Offline-first proxy for legacy clients.
* `TauriAPP/` - Modern management dashboard.
* `Win32APP/` - Native Windows 98/2000/XP client.

## Quick Start Order

To run the full stack locally:

1.  **Environment Setup:** 
    ```bash
    # Copy the example env file and set your API token
    cp .env.example .env
    # Edit .env and set RETLISTER_API_TOKEN=your-secret-token-here
    ```

2.  **Database & Backend:** Start `RustServer` to initialize `retlister.db`.
3.  **Proxy:** Start `ProxyService` (ensure it can reach the Backend).
4.  **Modern UI:** Launch `TauriAPP`.
5.  **Legacy UI:** Run `Win32APP` (configured to point to the Proxy).

Refer to the `README.md` in each subdirectory for specific build and configuration instructions.