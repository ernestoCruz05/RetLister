# RetLister Win32 Application

Native Windows XP/2000 client for managing wood leftovers through ProxyService.

## Features

- List all wood pieces from inventory
- Remove selected items
- Windows Classic UI (compatible with XP)
- Connects to ProxyService on Windows 11 host

## Building

### Visual Studio Command Prompt

```batch
build.bat
```

### Manual Compilation

```batch
cl.exe /EHsc /O2 RetLister.cpp /Fe:RetLister.exe /link user32.lib gdi32.lib comctl32.lib wininet.lib shlwapi.lib
```

### MinGW

```batch
g++ -O2 -mwindows RetLister.cpp -o RetLister.exe -lcomctl32 -lwininet -lshlwapi
```

## Configuration

Edit `RetLister.cpp` and change the PROXY_URL:

```cpp
#define PROXY_URL "http://192.168.56.1:8001"
```

Set this to your Windows 11 machine's IP address on the Ethernet network.

## Requirements

- Windows XP SP3 or later
- Network access to ProxyService
- Internet Explorer 6+ (for WinINet)

## Current Status

Implemented:
- List view with all resto data
- Refresh functionality
- Delete selected item
- Status bar with count

Coming soon:
- Add new resto dialog
- Search dialog
- Edit functionality
- Connection diagnostics
