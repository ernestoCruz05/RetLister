# RetLister

## Structure
- FlutterAPP → Mobile apps (iOS/Android)
- TauriAPP → Windows 11 desktop app
- RustServer → Backend API (Axum)
- Win32APP → Windows XP C client

## Deployment
- Server runs on Rust Axum with SQLite
- W11 proxy integrated into Tauri (Rust IPC)
- XP communicates by TCP → W11 Proxy
- Mobile apps connect using HTTPS

## Build Instructions
- Flutter: flutter run
- Tauri: npm run tauri dev
- RustServer: cargo run
- Win32APP: cl main.c /link user32.lib ws2_32.lib
