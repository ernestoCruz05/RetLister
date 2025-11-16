# RetLister Tauri Application

Windows desktop application for managing wood leftovers with Windows Classic UI styling.

## Features

### Restos Management
- Add, edit, delete wood pieces with dimensions and material info
- Multi-select with Ctrl+Click for bulk operations
- Quick filters by material and thickness
- Resizable columns
- Double-click to edit
- Keyboard shortcuts (Del, Enter, Esc)
- Case-insensitive search with auto-selection

### Statistics Dashboard
- Total inventory count and area
- Breakdown by material type
- Breakdown by thickness

### Cutting Optimizer
- Guillotine bin packing algorithm
- Multi-strategy optimization (area, width, height, perimeter)
- Configurable saw kerf width
- Minimum remainder constraints
- SVG visualization of cutting layouts
- Confirm/discard workflow

### Settings
- Kerf width (default: 3mm)
- Minimum remainder dimensions (default: 300x300mm)

## Design

Windows 2000/XP enterprise aesthetic with:
- Tahoma 12px font
- 3D beveled controls
- Folder-style tabs
- Classic color scheme (#d4d0c8)

## Running

```powershell
cd TauriAPP
npm install
npm run tauri dev
```

## Building

```powershell
npm run tauri build
```

## Requirements

- Node.js 18+
- Rust 1.70+
- RetLister server running on `http://localhost:8000`
