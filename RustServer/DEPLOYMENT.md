# Deployment Guide: RustServer with TLS

This guide covers secure deployment options for hosting RetLister externally.

## ⚠️ IMPORTANT: Do NOT Expose RustServer Directly

Never port-forward the Rust server directly to the internet. Always use a reverse proxy.

---

## Recommended Architecture

```
Internet → Domain (DNS) → Reverse Proxy (TLS) → RustServer (localhost:8000)
```

**Why?**
- ✅ Automatic HTTPS with valid certificates
- ✅ Hide your home IP behind a domain/Cloudflare
- ✅ Easy rate limiting, IP filtering, DDoS protection
- ✅ Keep RustServer simple (no TLS code)

---

## Option 1: Caddy Reverse Proxy (Recommended - Easiest)

### Install Caddy

**Windows:**
```powershell
winget install Caddy.Caddy
```

**Linux:**
```bash
sudo apt install -y debian-keyring debian-archive-keyring apt-transport-https
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | sudo tee /etc/apt/sources.list.d/caddy-stable.list
sudo apt update
sudo apt install caddy
```

### Caddyfile

Create `Caddyfile` in your server directory:

```caddy
# Replace with your domain
retlister.yourdomain.com {
    # Automatic HTTPS with Let's Encrypt
    
    # Forward to local RustServer
    reverse_proxy localhost:8000
    
    # Optional: Rate limiting
    rate_limit {
        zone dynamic {
            key {remote_host}
            events 100
            window 1m
        }
    }
    
    # Optional: IP whitelist (only allow specific IPs)
    # @blocked not remote_ip 203.0.113.0/24
    # respond @blocked 403
    
    # Logging
    log {
        output file /var/log/caddy/retlister.log
        format json
    }
}
```

### Start Caddy

**Windows:**
```powershell
# As administrator
cd C:\path\to\your\server
caddy run
```

**Linux (systemd):**
```bash
sudo systemctl enable --now caddy
```

### Configure RustServer

```powershell
# Set auth token
$env:AUTH_TOKEN = "your-secret-token-here-make-it-long-and-random"

# Start RustServer (only listens on localhost)
cargo run --release
```

### Test

```bash
# Public endpoint (no auth)
curl https://retlister.yourdomain.com/health

# Protected endpoint (requires token)
curl -H "Authorization: Bearer your-secret-token-here" https://retlister.yourdomain.com/list
```

---

## Option 2: Nginx Reverse Proxy

### Install Nginx

**Linux:**
```bash
sudo apt install nginx certbot python3-certbot-nginx
```

### Nginx Config

Create `/etc/nginx/sites-available/retlister`:

```nginx
server {
    listen 80;
    server_name retlister.yourdomain.com;
    
    # Redirect HTTP to HTTPS
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name retlister.yourdomain.com;
    
    # SSL certificates (managed by certbot)
    ssl_certificate /etc/letsencrypt/live/retlister.yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/retlister.yourdomain.com/privkey.pem;
    
    # Modern TLS config
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_prefer_server_ciphers on;
    ssl_ciphers 'ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256';
    
    # Proxy to RustServer
    location / {
        proxy_pass http://localhost:8000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # Timeouts
        proxy_connect_timeout 30s;
        proxy_send_timeout 30s;
        proxy_read_timeout 30s;
    }
    
    # Rate limiting
    limit_req_zone $binary_remote_addr zone=api:10m rate=10r/s;
    limit_req zone=api burst=20 nodelay;
}
```

### Enable and Get Certificate

```bash
sudo ln -s /etc/nginx/sites-available/retlister /etc/nginx/sites-enabled/
sudo certbot --nginx -d retlister.yourdomain.com
sudo systemctl reload nginx
```

---

## DNS & Port Forwarding

### 1. Get a Domain

- Cloudflare (free DNS, DDoS protection)
- Namecheap, Google Domains, etc.

### 2. Configure DNS

Point your domain to your home IP:

```
A record: retlister.yourdomain.com → YOUR_HOME_IP
```

**Optional: Use Cloudflare Proxy**
- Enable orange cloud in Cloudflare
- Hides your real IP
- Free DDoS protection
- But requires Cloudflare SSL mode (Full or Full Strict)

### 3. Port Forwarding on Router

Forward port **80** and **443** to your server's local IP:

```
External Port 80 → Internal IP (your server) Port 80
External Port 443 → Internal IP (your server) Port 443
```

---

## Security Checklist

### ✅ Essential
- [ ] Use reverse proxy (Caddy/nginx), never expose RustServer directly
- [ ] Set `AUTH_TOKEN` environment variable
- [ ] Use HTTPS (Let's Encrypt via reverse proxy)
- [ ] Keep auth token secret and long (32+ characters)
- [ ] Run automated backups (see `backup.ps1`)

### ✅ Recommended
- [ ] Use Cloudflare proxy to hide home IP
- [ ] Enable rate limiting in reverse proxy
- [ ] Monitor logs for suspicious activity
- [ ] Set up firewall rules (allow only 80/443)
- [ ] Use fail2ban (Linux) to block brute force

### ✅ Optional but Good
- [ ] IP whitelist (if you have static IPs)
- [ ] Two-factor auth (e.g., Authelia in front of Caddy)
- [ ] Monitoring (Uptime Kuma, Grafana)

---

## Example: Full Setup on Ubuntu Server

```bash
# 1. Install dependencies
sudo apt update
sudo apt install -y caddy sqlite3

# 2. Clone and build RustServer
cd /opt
git clone https://github.com/YOUR_USERNAME/RetLister.git
cd RetLister/RustServer
cargo build --release

# 3. Create systemd service
sudo tee /etc/systemd/system/retlister.service > /dev/null <<EOF
[Unit]
Description=RetLister API Server
After=network.target

[Service]
Type=simple
User=retlister
WorkingDirectory=/opt/RetLister/RustServer
Environment="AUTH_TOKEN=your-secret-token-here"
Environment="SERVER_LOG=info"
ExecStart=/opt/RetLister/RustServer/target/release/retlister-server
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

# 4. Create user and set permissions
sudo useradd -r -s /bin/false retlister
sudo chown -R retlister:retlister /opt/RetLister/RustServer/data

# 5. Configure Caddy (create Caddyfile as shown above)

# 6. Start services
sudo systemctl enable --now retlister
sudo systemctl enable --now caddy

# 7. Set up automated backups
sudo crontab -e
# Add: 0 2 * * * /opt/RetLister/RustServer/backup.ps1 -BackupType daily
# Add: 0 3 * * 0 /opt/RetLister/RustServer/backup.ps1 -BackupType weekly
```

---

## Client Configuration

### Update Proxy URL (XP Client)

In `Win32APP/RetLister_fixed.c`:
```c
#define PROXY_URL "https://retlister.yourdomain.com"
```

### Update Tauri App

In `TauriAPP/src/api.js`:
```javascript
const API_BASE = 'https://retlister.yourdomain.com';
const AUTH_TOKEN = 'your-secret-token-here';

// Add to all fetch calls:
headers: {
  'Authorization': `Bearer ${AUTH_TOKEN}`
}
```

---

## Troubleshooting

### Certificate Issues
```bash
# Check certificate
sudo certbot certificates

# Renew manually
sudo certbot renew --dry-run
```

### Connection Refused
```bash
# Check RustServer is running
curl http://localhost:8000/health

# Check reverse proxy
sudo systemctl status caddy
sudo tail -f /var/log/caddy/retlister.log
```

### Authentication Failures
- Verify `AUTH_TOKEN` env var is set
- Check token matches in client requests
- Look for "Authentication failed" in logs
