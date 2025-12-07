# RetLister Tauri Client

The modern dashboard for the RetLister system, built as a native Windows 11 desktop application using Tauri. It provides advanced visualization and management capabilities that extend beyond the legacy client's feature set.

## Tech Stack

* **Core:** [Tauri v2](https://tauri.app/) (Rust)
* **Frontend:** React 19
* **Build Tool:** Vite
* **Visualization:** Three.js with `@react-three/fiber` and `@react-three/drei`.
* **UI/UX:** Custom CSS implementation of the "Windows Classic" aesthetic to maintain visual consistency with the legacy Win32 app.

## Features

### Inventory Management
* **CRUD Operations:** Full creation, reading, updating, and deletion of inventory items.
* **Bulk Actions:** Multi-select support (Ctrl+Click) for batch deletion or modification.
* **Advanced Filtering:** Client-side sorting and filtering by material, thickness, and dimensions.

### 3D & 2D Visualization
* **Cutting Optimizer:** Interacts with the backend's optimization engine to display 2D cutting layouts for panel saws.
* **Cargo Loading:** Uses Three.js to render 3D packing of wood remnants into transport vans, helping visualize space utilization.

## Prerequisites

* **Node.js:** v18 or newer.
* **Rust:** Stable toolchain (1.70+).
* **Backend:** The `RustServer` must be running on `http://localhost:8000` for API calls to succeed.

## Development

1.  **Install Frontend Dependencies:**
    ```powershell
    npm install
    ```

2.  **Run in Development Mode:**
    This starts the Vite dev server and the Tauri window.
    ```powershell
    npm run tauri dev
    ```

## Building for Production

To create a standalone Windows installer (`.msi`) or executable (`.exe`):

```powershell
npm run tauri build