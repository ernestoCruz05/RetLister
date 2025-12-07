# RetLister Win32 Client

A native Windows application written in C (Win32 API) designed for legacy hardware (Windows 98/2000/XP). It eliminates the need for heavy runtimes like .NET or Java, ensuring smooth performance on constrained hardware.

It communicates exclusively with the **ProxyService** (running on a modern host) to bridge the gap between legacy network stacks and the modern REST API.

## Architecture

* **Language:** C (C99 standard compatible)
* **GUI:** Pure Win32 API (User32, GDI32, Comctl32)
* **Network:** WinInet (Internet Explorer network stack)
* **Rendering:** Custom GDI routines for visualizing cutting plans without OpenGL/DirectX.

## Features

* **Inventory Management:**
  * **Retalhos:** Tabular view of all wood remnants with sortable columns.
  * **Search:** Filter inventory by dimensions and material.
* **Cutting Optimization:**
  * **Visualizer:** Renders 2D cutting layouts directly on the GDI canvas.
  * **Workflow:** Supports adding cut requests, running the optimizer (via Proxy), and confirming stock deduction.
* **System Status:**
  * Real-time monitoring of Proxy and Main Server connectivity.
  * Offline detection.

## Configuration Files

The application looks for configuration files in the executable's directory. These are created automatically if they do not exist.

* **`proxy.cfg`**: Contains the URL of the Proxy Service.
  * Default: `http://192.168.56.1:8001`
  * Format: Plain text URL (e.g., `http://192.168.1.10:8001`)
* **`ui.cfg`**: Stores persistent UI preferences.
  * Font size settings.
  * Column widths for the inventory list.

## Build Instructions

The source code `RetLister.c` is single-file and compiler-agnostic.

### Visual Studio (MSVC)
I could not get this too work, it should be possible but i find the pelles C much easier...

### Pelles
A build_pelles.bat script is included for users of Pelles C, a popular environment for maintaining legacy Windows software.