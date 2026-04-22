# Knowledge Vault Operations Guide

**Version:** 1.0  
**Last Updated:** 2026-04-21  
**Audience:** System Operators, DevOps Engineers

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Installation](#installation)
3. [Configuration](#configuration)
4. [Starting the Service](#starting-the-service)
5. [Monitoring and Health Checks](#monitoring-and-health-checks)
6. [Troubleshooting](#troubleshooting)
7. [Maintenance](#maintenance)
8. [Backup and Recovery](#backup-and-recovery)
9. [Scaling Considerations](#scaling-considerations)

---

## Prerequisites

### System Requirements

| Requirement | Specification |
|-------------|---------------|
| OS | Linux (ARM64 or x86_64) |
| Architecture | aarch64 or x86_64 |
| Memory | 2 GB minimum (4 GB recommended) |
| Storage | 500 MB minimum (SSD recommended) |
| Network | Outbound HTTPS for Gemini API |

### Required Environment Variables

These variables must be set before starting the service:

| Variable | Purpose | Required | Format | Example |
|----------|---------|----------|--------|---------|
| `KV_JWT_SECRET` | HS256 signing key | Yes | String, 32+ chars | `my-super-secret-key-min-32-chars` |
| `KV_GEMINI_API_KEY` | Google Gemini API key | Yes | API key | `AIzaSyD...` |
| `KV_DATA_DIR` | Data storage directory | No | Path | `/var/lib/knowledge-vault` |
| `KV_PORT` | HTTP server port | No | Integer | `8080` |

### Third-party API Access

- **Google Gemini API**: Ensure your API key has quota for `generativelanguage.googleapis.com`
- **Open Library API**: No authentication required, but rate-limited to ~1 req/sec (respected by default)

---

## Installation

### 1. Download the Binary

Download the latest `knowledge-vault` binary from GitHub Releases:

```bash
# Download the binary for your architecture
curl -L https://github.com/renatobardi/gist/releases/latest/download/knowledge-vault \
  -o /tmp/knowledge-vault

# Verify the download
file /tmp/knowledge-vault
```

### 2. Install to System Path

```bash
# Copy to system location (requires sudo)
sudo install -m 755 /tmp/knowledge-vault /usr/local/bin/knowledge-vault

# Verify installation
which knowledge-vault
knowledge-vault --version  # If version flag is supported
```

### 3. Create Service User and Data Directory

```bash
# Create a dedicated user (non-login)
sudo useradd --system --no-create-home --shell /usr/sbin/nologin knowledge-vault

# Create data directory
sudo mkdir -p /var/lib/knowledge-vault

# Set ownership
sudo chown knowledge-vault:knowledge-vault /var/lib/knowledge-vault

# Set permissions (user: rwx, others: none)
sudo chmod 700 /var/lib/knowledge-vault
```

---

## Configuration

### Option 1: Environment Variables (Recommended)

Set variables in your shell session or via systemd service file:

```bash
export KV_JWT_SECRET="your-secret-key-min-32-characters-long"
export KV_GEMINI_API_KEY="AIzaSyD..."
export KV_DATA_DIR="/var/lib/knowledge-vault"
export KV_PORT="8080"
```

### Option 2: Systemd Service File

Create `/etc/systemd/system/knowledge-vault.service`:

```ini
[Unit]
Description=Knowledge Vault Service
After=network.target

[Service]
Type=simple
User=knowledge-vault
Group=knowledge-vault
WorkingDirectory=/var/lib/knowledge-vault
ExecStart=/usr/local/bin/knowledge-vault
Restart=on-failure
RestartSec=5

# Environment variables
Environment="KV_JWT_SECRET=your-secret-key-min-32-characters-long"
Environment="KV_GEMINI_API_KEY=AIzaSyD..."
Environment="KV_DATA_DIR=/var/lib/knowledge-vault"
Environment="KV_PORT=8080"

# Resource limits (optional)
CPUQuota=200%
MemoryLimit=512M

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=knowledge-vault

[Install]
WantedBy=multi-user.target
```

### Option 3: TLS Termination (Recommended for Production)

Knowledge Vault does not include built-in TLS. Use a reverse proxy:

**Nginx Configuration:**

```nginx
server {
    listen 443 ssl http2;
    server_name vault.example.com;

    ssl_certificate /etc/letsencrypt/live/vault.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/vault.example.com/privkey.pem;

    # Security headers
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
    add_header X-Frame-Options "DENY" always;
    add_header X-Content-Type-Options "nosniff" always;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # WebSocket support
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}

# Redirect HTTP to HTTPS
server {
    listen 80;
    server_name vault.example.com;
    return 301 https://$server_name$request_uri;
}
```

---

## Starting the Service

### Using Systemd

```bash
# Enable service to start on boot
sudo systemctl enable knowledge-vault

# Start the service
sudo systemctl start knowledge-vault

# Verify it's running
sudo systemctl status knowledge-vault

# View recent logs
sudo journalctl -u knowledge-vault -n 50
```

### Manual Startup (Development)

```bash
# Set environment variables
export KV_JWT_SECRET="dev-secret-min-32-characters-here"
export KV_GEMINI_API_KEY="your-api-key"
export KV_DATA_DIR="./data"
export KV_PORT="8080"

# Run the binary
/usr/local/bin/knowledge-vault
```

### Expected Startup Output

```
[INFO] Knowledge Vault v0.1.0 starting up...
[INFO] Extracting NATS server binary...
[INFO] NATS server started on 127.0.0.1:4222
[INFO] SurrealDB connected
[INFO] Running schema migrations...
[INFO] HTTP server listening on 0.0.0.0:8080
[INFO] Ready to accept connections
```

---

## Monitoring and Health Checks

### Health Endpoint

Check the service health without authentication:

```bash
curl http://localhost:8080/health

# Expected response:
{
  "status": "ok",
  "version": "0.1.0",
  "db": "connected"
}
```

### Systemd Service Status

```bash
# Check service state
systemctl is-active knowledge-vault
# Output: active

# Check service enabled on boot
systemctl is-enabled knowledge-vault
# Output: enabled

# Full status
systemctl status knowledge-vault
```

### Log Monitoring

```bash
# View last 50 lines
journalctl -u knowledge-vault -n 50

# Follow logs in real-time
journalctl -u knowledge-vault -f

# View logs from last hour
journalctl -u knowledge-vault --since "1 hour ago"

# View logs with log level
journalctl -u knowledge-vault -p err  # errors only
journalctl -u knowledge-vault -p info # info and above
```

### Prometheus Metrics (Future)

Currently, the application outputs logs to JSON format for structured log aggregation. Prometheus metrics are planned for v2.

### Alert Conditions

Set up monitoring alerts for:

1. **Service Down**: Service not active for > 2 minutes
2. **Database Disconnected**: `/health` returns `db: disconnected`
3. **Memory Usage**: > 80% of configured `MemoryLimit`
4. **Disk Space**: Data directory inode usage > 80%
5. **Gemini API Failures**: Error rate > 5% in past 5 minutes

---

## Troubleshooting

### Service Won't Start

**Symptoms**: `systemctl status knowledge-vault` shows failed state

**Steps**:

```bash
# 1. Check recent logs for error messages
journalctl -u knowledge-vault -n 30

# 2. Verify environment variables are set
sudo systemctl cat knowledge-vault

# 3. Test binary directly with environment variables
sudo -u knowledge-vault env KV_JWT_SECRET="..." KV_GEMINI_API_KEY="..." \
  /usr/local/bin/knowledge-vault

# 4. Check file permissions
ls -la /var/lib/knowledge-vault
sudo chown -R knowledge-vault:knowledge-vault /var/lib/knowledge-vault
```

### Database Connection Failed

**Symptoms**: Logs show "SurrealDB connection failed"

**Causes & Solutions**:

- **Data directory corrupted**: Remove SurrealKV directory and restart
  ```bash
  sudo rm -rf /var/lib/knowledge-vault/knowledge_vault.surrealkv
  sudo systemctl restart knowledge-vault
  ```

- **Insufficient disk space**: Check available space
  ```bash
  df -h /var/lib/knowledge-vault
  ```

- **Permission issues**: Verify service user owns data directory
  ```bash
  sudo chown knowledge-vault:knowledge-vault /var/lib/knowledge-vault -R
  sudo chmod 700 /var/lib/knowledge-vault
  ```

### High Memory Usage

**Diagnosis**:

```bash
# Check process memory
ps aux | grep knowledge-vault

# Check system memory
free -h

# Check if NATS server is consuming memory
ps aux | grep nats-server
```

**Solutions**:

- Increase `MemoryLimit` in systemd service file
- Restart service to clear in-memory caches
- Check for memory leaks in application logs

### API Requests Failing (401 Unauthorized)

**Causes**:

- JWT token expired (24-hour expiry)
- PAT token revoked or invalid
- Token not passed correctly in `Authorization` header

**Test with curl**:

```bash
# Login first
TOKEN=$(curl -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"admin@example.com","password":"your-password"}' \
  | jq -r '.token')

# Use token for subsequent requests
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/works
```

### NATS Consumer Not Processing Messages

**Symptoms**: Works remain in "pending" status indefinitely

**Diagnosis**:

```bash
# Check NATS process is running
ps aux | grep nats-server

# Check NATS logs (written to stdout of knowledge-vault)
journalctl -u knowledge-vault | grep -i nats

# Verify JetStream stream exists
# (Requires NATS CLI tool)
nats -s nats://127.0.0.1:4222 stream list
```

**Recovery**:

```bash
# Restart the service to reinitialize NATS
sudo systemctl restart knowledge-vault
```

---

## Maintenance

### Regular Tasks

**Daily**:
- Monitor logs for errors
- Check disk space: `df -h /var/lib/knowledge-vault`

**Weekly**:
- Review health endpoint for degraded states
- Check log file size (logs are rotated by systemd journal)

**Monthly**:
- Audit API token usage: Check for unused PATs and revoke them
- Verify Gemini API quota hasn't been exceeded
- Review systemd service logs for warnings

### Upgrading the Binary

1. **Download new version**:
   ```bash
   curl -L https://github.com/renatobardi/gist/releases/latest/download/knowledge-vault \
     -o /tmp/knowledge-vault-new
   ```

2. **Backup current binary**:
   ```bash
   sudo cp /usr/local/bin/knowledge-vault /usr/local/bin/knowledge-vault.backup
   ```

3. **Replace binary**:
   ```bash
   sudo install -m 755 /tmp/knowledge-vault-new /usr/local/bin/knowledge-vault
   ```

4. **Restart service**:
   ```bash
   sudo systemctl restart knowledge-vault
   ```

5. **Verify**:
   ```bash
   curl http://localhost:8080/health
   ```

6. **Rollback if needed**:
   ```bash
   sudo cp /usr/local/bin/knowledge-vault.backup /usr/local/bin/knowledge-vault
   sudo systemctl restart knowledge-vault
   ```

### Cleaning Old Logs

The systemd journal is managed automatically by `journald`. To limit disk space:

```bash
# Limit journal to 500 MB
sudo bash -c 'echo "SystemMaxUse=500M" >> /etc/systemd/journald.conf'

# Apply changes
sudo systemctl restart systemd-journald
```

---

## Backup and Recovery

### Data Files

Knowledge Vault stores all data in the directory specified by `KV_DATA_DIR`:

```
KV_DATA_DIR/
├── knowledge_vault.surrealkv/   (SurrealDB embedded database)
└── nats/                        (NATS JetStream persistent storage)
```

### Backup Procedure

```bash
# Stop the service (optional, but recommended for consistency)
sudo systemctl stop knowledge-vault

# Create backup
sudo tar -czf /backup/knowledge-vault-$(date +%Y-%m-%d).tar.gz \
  /var/lib/knowledge-vault/

# Verify backup
tar -tzf /backup/knowledge-vault-*.tar.gz | head

# Restart service
sudo systemctl start knowledge-vault
```

### Automated Daily Backup (Optional)

Create `/etc/cron.daily/knowledge-vault-backup`:

```bash
#!/bin/bash

BACKUP_DIR="/backup/knowledge-vault"
KEEP_DAYS=30

# Create backup directory
mkdir -p "$BACKUP_DIR"

# Backup data
tar -czf "$BACKUP_DIR/knowledge-vault-$(date +\%Y-\%m-\%d).tar.gz" \
  /var/lib/knowledge-vault/ 2>/dev/null

# Remove old backups (keep 30 days)
find "$BACKUP_DIR" -name "*.tar.gz" -mtime +$KEEP_DAYS -delete
```

```bash
sudo chmod +x /etc/cron.daily/knowledge-vault-backup
```

### Recovery Procedure

```bash
# Stop the service
sudo systemctl stop knowledge-vault

# Remove corrupted data
sudo rm -rf /var/lib/knowledge-vault/*

# Restore from backup
sudo tar -xzf /backup/knowledge-vault-YYYY-MM-DD.tar.gz -C /

# Fix permissions
sudo chown -R knowledge-vault:knowledge-vault /var/lib/knowledge-vault

# Restart service
sudo systemctl start knowledge-vault

# Verify
curl http://localhost:8080/health
```

---

## Scaling Considerations

### Single-User Design

Knowledge Vault v1 is designed for **single-user** operation. No multi-user isolation is implemented in the data model. For multi-user deployments, consider:

1. **Running multiple instances** with separate data directories (one per user)
2. **Load balancing** across instances (using nginx or haproxy)
3. **Shared reverse proxy** for TLS termination

### Resource Planning

| Metric | Expected | Limiting Factor |
|--------|----------|-----------------|
| Binary size | ~55 MB | Download, storage |
| Memory footprint | 100-300 MB | SurrealDB buffer pool, NATS |
| Concurrent users | 1 | Application design |
| Books (max) | 100 | Tested and validated |
| Concepts (expected) | 500-2000 | Per 100 books |

### Performance Optimization

**Cold Start Time**: ~2 seconds (NATS extraction + initialization)

If cold start time is critical:
1. Pre-extract NATS binary to avoid startup delay
2. Warm up database after first start

**Database Performance**: All query patterns are optimized for < 50ms on a single book's concept graph. For full-graph traversals with 100 books, expect < 1s response time.

---

## Disaster Recovery Plan (DRP)

### RTO/RPO Targets

| Metric | Target | Implementation |
|--------|--------|-----------------|
| RTO (Recovery Time Objective) | 5 minutes | Service restart + data recovery from backup |
| RPO (Recovery Point Objective) | 24 hours | Daily backups with 30-day retention |

### Recovery Scenarios

**Scenario 1: Service Crashes**
- Time to recovery: 1 minute (systemd auto-restart with 5s delay)

**Scenario 2: Data Corruption**
- Time to recovery: 5-10 minutes (restore from backup)

**Scenario 3: Hardware Failure**
- Time to recovery: 1 hour (provision new VM, restore binary, restore data)

### Post-Incident Checklist

1. Verify service is operational: `curl http://localhost:8080/health`
2. Check data consistency: Query a known work and verify concepts are present
3. Review logs for root cause: `journalctl -u knowledge-vault --since "1 hour ago"`
4. Update incident ticket with timeline and resolution
5. Perform full backup after recovery

---

*End of Operations Guide v1.0 — Knowledge Vault*
