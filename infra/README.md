# Knowledge Vault Infrastructure

This directory contains infrastructure-as-code and operational documentation for deploying and managing the Knowledge Vault service on Oracle Cloud ARM64 VMs.

## Deployment Architecture

Knowledge Vault runs as a single systemd service (`knowledge-vault.service`) on a VM with:
- **Compute target:** `aarch64-unknown-linux-musl` (Oracle Cloud ARM64 Free Tier)
- **Binary location:** `/usr/local/bin/knowledge-vault`
- **Data directory:** `/var/lib/knowledge-vault/`
- **Configuration:** `/etc/knowledge-vault/env` (secrets and overrides)
- **HTTP port:** `8080` (TLS termination handled by reverse proxy)

## Initial Setup (One-time)

### Prerequisites
- Oracle Cloud ARM64 VM running Ubuntu 22.04 LTS or later
- SSH access to the VM
- `sudo` privileges

### 1. Create Service User
```bash
sudo useradd -r -s /bin/false -d /var/lib/knowledge-vault knowledge-vault
```

### 2. Create Directories
```bash
sudo mkdir -p /var/lib/knowledge-vault
sudo mkdir -p /etc/knowledge-vault
sudo chown knowledge-vault:knowledge-vault /var/lib/knowledge-vault
sudo chown root:root /etc/knowledge-vault
sudo chmod 750 /etc/knowledge-vault
```

### 3. Prepare Environment File
Create `/etc/knowledge-vault/env` with required secrets:
```bash
sudo tee /etc/knowledge-vault/env > /dev/null <<'EOF'
KV_JWT_SECRET=<generate-a-random-64-char-string>
KV_GEMINI_API_KEY=<your-gemini-api-key>
KV_PORT=8080
KV_DATA_DIR=/var/lib/knowledge-vault
EOF
```

Make the file readable only by the service user:
```bash
sudo chown root:knowledge-vault /etc/knowledge-vault/env
sudo chmod 640 /etc/knowledge-vault/env
```

### 4. Install Systemd Unit File
```bash
sudo cp infra/knowledge-vault.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable knowledge-vault.service
```

## Deployment (Binary Replacement)

The CD pipeline (GitHub Actions) automatically builds the binary and uploads it to GitHub Releases. To deploy manually:

### 1. Download the Latest Binary
```bash
cd /tmp
wget https://github.com/renatobardi/gist/releases/download/v<VERSION>/knowledge-vault-aarch64
chmod +x knowledge-vault-aarch64
```

### 2. Backup Current Binary
```bash
sudo cp /usr/local/bin/knowledge-vault /usr/local/bin/knowledge-vault.backup-<TIMESTAMP>
```

Example:
```bash
sudo cp /usr/local/bin/knowledge-vault /usr/local/bin/knowledge-vault.backup-$(date +%Y%m%d-%H%M%S)
```

### 3. Install New Binary
```bash
sudo mv /tmp/knowledge-vault-aarch64 /usr/local/bin/knowledge-vault
sudo chown root:root /usr/local/bin/knowledge-vault
sudo chmod 755 /usr/local/bin/knowledge-vault
```

### 4. Restart Service
```bash
sudo systemctl restart knowledge-vault
```

### 5. Verify Deployment
```bash
sudo systemctl status knowledge-vault

# Check service logs
sudo journalctl -u knowledge-vault -n 50

# Verify health endpoint
curl -s http://localhost:8080/health | jq .
```

Expected health response:
```json
{
  "status": "ok",
  "version": "0.1.0",
  "db": "connected"
}
```

## Service Management Commands

### Start the Service
```bash
sudo systemctl start knowledge-vault
```

### Stop the Service
```bash
sudo systemctl stop knowledge-vault
```

### Restart the Service
```bash
sudo systemctl restart knowledge-vault
```

### Check Service Status
```bash
sudo systemctl status knowledge-vault
```

### View Real-time Logs
```bash
sudo journalctl -u knowledge-vault -f
```

### View Last 100 Log Lines
```bash
sudo journalctl -u knowledge-vault -n 100
```

### View Logs Since Last Boot
```bash
sudo journalctl -u knowledge-vault -b
```

### View Logs for a Specific Time Range
```bash
sudo journalctl -u knowledge-vault --since "2 hours ago"
sudo journalctl -u knowledge-vault --since 2026-04-20 --until 2026-04-21
```

### Check Service Enable Status
```bash
sudo systemctl is-enabled knowledge-vault
```

### Enable Service Auto-start on Boot
```bash
sudo systemctl enable knowledge-vault
```

### Disable Service Auto-start
```bash
sudo systemctl disable knowledge-vault
```

### Reload Configuration Without Restart
To apply environment file changes without restarting:
```bash
sudo systemctl daemon-reload
sudo systemctl restart knowledge-vault
```

## Health Checks and Monitoring

### Manual Health Check
```bash
curl -s http://localhost:8080/health | jq .
```

### Automated Health Check (for load balancers / monitoring)
Configure your reverse proxy (nginx, caddy) with health checks pointing to `http://localhost:8080/health`.

The endpoint returns:
- **200 OK** with `{"status": "ok", "db": "connected"}` when healthy
- **503 Service Unavailable** with `{"status": "degraded", "db": "disconnected"}` when degraded (database unreachable)

### Service Restart Configuration
The systemd unit is configured with:
- **Restart strategy:** Restart on failure (`Restart=on-failure`)
- **Restart delay:** 5 seconds between attempts (`RestartSec=5`)
- **Burst limit:** 3 restarts within 60 seconds (`StartLimitBurst=3`)
- **Burst interval:** 60 seconds (`StartLimitInterval=60`)

If the service fails more than 3 times in 60 seconds, systemd will not attempt further restarts. Manual intervention is required.

## Rollback Procedures

### Quick Rollback to Previous Binary
If the new deployment has issues and you've created a backup:

```bash
sudo cp /usr/local/bin/knowledge-vault.backup-<TIMESTAMP> /usr/local/bin/knowledge-vault
sudo systemctl restart knowledge-vault
```

Example:
```bash
# List available backups
ls -la /usr/local/bin/knowledge-vault.backup-*

# Restore a specific backup
sudo cp /usr/local/bin/knowledge-vault.backup-20260420-143022 /usr/local/bin/knowledge-vault
sudo systemctl restart knowledge-vault
```

### Verify Rollback
```bash
# Check the binary version
/usr/local/bin/knowledge-vault --version

# Verify service is healthy
curl -s http://localhost:8080/health | jq .

# View logs
sudo journalctl -u knowledge-vault -n 20
```

### Full Rollback with Database Snapshot
If the new version created incompatible data:

1. **Stop the service:**
   ```bash
   sudo systemctl stop knowledge-vault
   ```

2. **Check available database snapshots:**
   ```bash
   ls -la /var/lib/knowledge-vault/
   ```

3. **Restore from backup (if available):**
   ```bash
   # SurrealDB backup location
   sudo rm -rf /var/lib/knowledge-vault/knowledge_vault.surrealkv
   sudo cp -r /var/lib/knowledge-vault/knowledge_vault.surrealkv.backup-<TIMESTAMP> /var/lib/knowledge-vault/knowledge_vault.surrealkv
   sudo chown -R knowledge-vault:knowledge-vault /var/lib/knowledge-vault
   ```

4. **Restore binary:**
   ```bash
   sudo cp /usr/local/bin/knowledge-vault.backup-<TIMESTAMP> /usr/local/bin/knowledge-vault
   ```

5. **Restart service:**
   ```bash
   sudo systemctl start knowledge-vault
   sudo systemctl status knowledge-vault
   ```

### Rollback Timeline Tracking
Always record deployment times in a log file for reference:

```bash
# Log deployment
echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) - Deployed version v0.1.0" | sudo tee -a /var/log/knowledge-vault-deployments.log
```

## Configuration

### Environment Variables
The service sources environment from `/etc/knowledge-vault/env`. Supported variables:

| Variable | Description | Required | Example |
|----------|-------------|----------|---------|
| `KV_JWT_SECRET` | Secret for JWT signing (64+ chars) | Yes | `random-string` |
| `KV_GEMINI_API_KEY` | Google Gemini API key | Yes | `AIza...` |
| `KV_PORT` | HTTP listen port | No | `8080` |
| `KV_DATA_DIR` | Data directory path | No | `/var/lib/knowledge-vault` |
| `KV_GEMINI_MODEL` | Gemini model name | No | `gemini-3.0-flash-preview` |

### Secrets Management Best Practices
- Never commit `/etc/knowledge-vault/env` to version control
- Rotate `KV_JWT_SECRET` every 90 days (requires restarting all sessions)
- Store `KV_GEMINI_API_KEY` in a secrets manager, load at deployment time
- Use `sudo` to protect `/etc/knowledge-vault/` access (owned by root, readable by service group)

## Post-Deployment Validation

### Health Check
```bash
curl -s http://localhost:8080/health | jq .
```

### Database Connectivity
The health endpoint verifies database connectivity. If `"db": "disconnected"`, check:
```bash
sudo journalctl -u knowledge-vault -n 50 | grep -i "db\|database\|surrealdb"
```

### API Endpoint Validation
```bash
# Check if /setup endpoint is accessible
curl -s http://localhost:8080/setup | head -20

# Verify WebSocket endpoint (returns 400 without upgrade header, which is expected)
curl -i -N -H "Connection: Upgrade" -H "Upgrade: websocket" http://localhost:8080/ws
```

## Disaster Recovery

### Service Won't Start
1. Check logs: `sudo journalctl -u knowledge-vault -n 100`
2. Common causes:
   - Missing `/etc/knowledge-vault/env`: Recreate with required vars
   - Corrupted database: Restore from backup or delete `/var/lib/knowledge-vault/knowledge_vault.surrealkv` to reset
   - Binary permissions: Ensure `/usr/local/bin/knowledge-vault` is executable

### Database Corruption
1. Stop service: `sudo systemctl stop knowledge-vault`
2. Backup current state: `sudo cp -r /var/lib/knowledge-vault /var/lib/knowledge-vault.corrupted-$(date +%s)`
3. Delete corrupted DB: `sudo rm -rf /var/lib/knowledge-vault/knowledge_vault.surrealkv`
4. Restart service (it will recreate schema): `sudo systemctl start knowledge-vault`
5. Verify: `curl -s http://localhost:8080/health`

### Port Already in Use
If port 8080 is in use:
```bash
# Find the process using port 8080
sudo lsof -i :8080

# Kill the process
sudo kill -9 <PID>

# Or change KV_PORT in /etc/knowledge-vault/env and restart
sudo systemctl restart knowledge-vault
```

## Metrics and Observability

The service outputs structured JSON logs to journald. View logs:
```bash
sudo journalctl -u knowledge-vault -o json | jq .
```

Log fields include:
- `timestamp`: RFC 3339 format
- `level`: ERROR, WARN, INFO, DEBUG
- `message`: Log message
- `target`: Rust module name
- `work_id`: For work-processing context

### Monitoring Integration
To integrate with a monitoring system (Prometheus, Grafana, etc.):
1. Expose metrics on a dedicated endpoint (future enhancement)
2. Use health check endpoint (`/health`) for liveness probes
3. Parse journald logs for SLO tracking

## Troubleshooting Checklist

- [ ] Service is running: `sudo systemctl status knowledge-vault`
- [ ] Health endpoint responds: `curl -s http://localhost:8080/health`
- [ ] Database is writable: Check latest logs for DB connection errors
- [ ] Logs show no errors: `sudo journalctl -u knowledge-vault -p err`
- [ ] Binary is executable: `ls -la /usr/local/bin/knowledge-vault`
- [ ] Service user exists: `id knowledge-vault`
- [ ] Data directory is writable: `ls -la /var/lib/knowledge-vault`
- [ ] Environment variables are set: `sudo systemctl show-environment -u knowledge-vault`

## References
- [systemd.service manual](https://man7.org/linux/man-pages/man5/systemd.service.5.html)
- [systemd.unit manual](https://man7.org/linux/man-pages/man5/systemd.unit.5.html)
- [Knowledge Vault Architecture](../_bmad/docs/architecture.md#7-deployment-topology)
- [Health Check API](../_bmad/docs/architecture.md#4-api-design)
