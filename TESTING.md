# Cross-Machine Testing Guide

## Network Setup Overview

```
┌─────────────────────────┐
│   Development PC        │
│   (Your PC)             │
│   RustServer :8000      │
│   TauriAPP              │
└───────────┬─────────────┘
            │ LAN
            │
┌───────────┴─────────────┐
│   Secretary PC (W11)    │
│   ProxyService :8001    │
│   Browser/Win32 Client  │
└───────────┬─────────────┘
            │ LAN
            │
┌───────────┴─────────────┐
│   Windows XP PC         │
│   Win32 Client          │
└─────────────────────────┘
```

## Step 1: Get Your Development PC's IP

On your development PC, run:
```powershell
ipconfig
```

Look for "IPv4 Address" on your active network adapter (e.g., `192.168.1.10`).

**Example Output:**
```
Ethernet adapter Ethernet:
   IPv4 Address. . . . . . . . . . . : 192.168.1.10
   Subnet Mask . . . . . . . . . . . : 255.255.255.0
```

**⚠️ Important:** Use the actual IP, not `192.168.1.10` - that's just an example!

---

## Step 2: Configure Firewall (Development PC)

Allow incoming connections on ports **8000** and **8001**:

```powershell
# Run as Administrator
New-NetFirewallRule -DisplayName "RetLister RustServer" -Direction Inbound -Protocol TCP -LocalPort 8000 -Action Allow
```

Or manually:
1. Open **Windows Defender Firewall** → Advanced Settings
2. **Inbound Rules** → New Rule → Port
3. TCP, Port **8000**
4. Allow the connection
5. Repeat for port **8001** if needed

---

## Step 3: Start RustServer (Development PC)

```powershell
cd C:\Users\fakyc\RetLister\RustServer
cargo run --release
```

✅ Server should show: `Server listening on 0.0.0.0:8000`

**Test from development PC:**
```powershell
curl http://localhost:8000/health
# Should return: {"status":"ok"}
```

---

## Step 4: Configure Secretary PC (W11)

### Option A: Run ProxyService on Secretary PC

1. **Copy ProxyService to Secretary PC:**
   - Build release: `cargo build --release` in `ProxyService/`
   - Copy `target/release/retlister-proxy.exe` to Secretary PC
   - Copy `data/` folder (for local SQLite database)

2. **Update ProxyService main server URL:**
   Edit `ProxyService/src/main.rs` line ~35:
   ```rust
   const MAIN_SERVER: &str = "http://192.168.1.10:8000"; // Use YOUR dev PC IP
   ```
   
   Rebuild: `cargo build --release`

3. **Create `config.json` on Secretary PC:**
   ```json
   {
     "main_server_url": "https://api.faky.dev"
   }
   ```

4. **Start ProxyService on Secretary PC:**
   ```cmd
   retlister-proxy.exe
   ```

5. **Configure Firewall on Secretary PC:**
   Allow port **8001** incoming (same as Step 2)

### Option B: Use TauriAPP with Dynamic URL

Since you already have the URL configuration feature in `api.js`:

1. **Build TauriAPP:**
   ```powershell
   cd TauriAPP
   npm run tauri build
   ```

2. **Copy installer to Secretary PC:**
   - Find installer in `TauriAPP/src-tauri/target/release/bundle/`
   - Install on Secretary PC

3. **On first launch:**
   - Go to **Estado** tab
   - Change "URL do Servidor" to `http://192.168.1.10:8000`
   - Click **Salvar**

---

## Step 5: Configure Windows XP Client

### Build Win32 Client

**For MSVC (Visual Studio):**
```powershell
cd Win32APP
.\build.bat
```

**For Pelles C (XP Compatible):**
```powershell
cd Win32APP
.\build_pelles.bat
```

### Deploy to Windows XP

1. **Copy `RetLister.exe` to XP machine**

2. **Edit server URL in the executable OR use config file**

   **Option A - Hardcode in source before building:**
   Edit `Win32APP/RetLister_fixed.c` line with `SERVER_URL`:
   ```c
   #define SERVER_URL "http://192.168.1.10:8000"
   ```

   **Option B - Create `config.ini`** (if you implement config loading):
   ```ini
   [Server]
   URL=http://192.168.1.10:8000
   ```

3. **Test from XP:**
   - Launch `RetLister.exe`
   - It should connect to your dev PC's RustServer

---

## Step 6: Test Network Connectivity

### From Secretary PC (W11):

```powershell
# Test RustServer
curl http://192.168.1.10:8000/health

# Test Proxy (if running on Secretary PC)
curl http://localhost:8001/health
```

### From Windows XP:

```cmd
ping 192.168.1.10
```

If ping works but HTTP doesn't, it's a firewall issue.

---

## Testing Checklist

- [ ] **Development PC**
  - [ ] RustServer running on `:8000`
  - [ ] Firewall allows port 8000
  - [ ] Can access `http://localhost:8000/health`
  
- [ ] **Secretary PC (W11)**
  - [ ] Can ping Development PC
  - [ ] Can access `http://192.168.1.10:8000/health`
  - [ ] TauriAPP installed and configured
  - [ ] ProxyService running (if using proxy)
  
- [ ] **Windows XP PC**
  - [ ] Can ping Development PC
  - [ ] Win32 client compiled
  - [ ] Client configured with correct server URL
  - [ ] Can add/list/edit items

---

## Common Issues

### "Connection Refused" Error
- ✅ Check firewall on development PC
- ✅ Verify server is running with `netstat -an | findstr 8000`
- ✅ Ensure using `0.0.0.0` not `127.0.0.1` in bind address

### "Cannot Connect to Server"
- ✅ Ping the development PC IP from client
- ✅ Check all devices are on same network/subnet
- ✅ Verify IP address hasn't changed (use static IP or DHCP reservation)

### Win32 Client Shows Blank Data
- ✅ Check server logs for incoming requests
- ✅ Verify JSON parsing works (test with Postman/curl first)
- ✅ Enable debug logging in Win32 client

### CORS Errors (Browser Clients)
- ✅ CORS is already configured in RustServer to allow all origins
- ✅ If issues persist, check browser console for exact error

---

## Production Deployment Notes

When ready for actual deployment (not testing):

1. **Static IP**: Assign static IPs or use DNS names
2. **HTTPS**: Add TLS certificates (Let's Encrypt + reverse proxy)
3. **Service**: Run servers as Windows Services (use NSSM)
4. **Database Backups**: Use the `backup.ps1` script in RustServer
5. **Logging**: Configure persistent logs (already using tracing)
6. **Monitoring**: Set up health check monitoring

---

## Quick Start Script

Create `start_testing.ps1` on Development PC:

```powershell
# Start RustServer
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd C:\Users\fakyc\RetLister\RustServer; cargo run --release"

# Wait for server to start
Start-Sleep -Seconds 3

# Get IP and show
$ip = (Get-NetIPAddress -AddressFamily IPv4 -InterfaceAlias "Ethernet").IPAddress
Write-Host "============================================" -ForegroundColor Green
Write-Host "RustServer running on: http://${ip}:8000" -ForegroundColor Cyan
Write-Host "Health check: curl http://${ip}:8000/health" -ForegroundColor Yellow
Write-Host "============================================" -ForegroundColor Green
```

Run: `.\start_testing.ps1`
