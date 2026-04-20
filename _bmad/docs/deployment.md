# Deployment Guide

## Overview

The Knowledge Vault binary is deployed to an Oracle Cloud ARM64 VM using the GitHub Actions CD pipeline. The deployment is automatic on every merge to `main` and includes health checks to verify successful startup.

## Deployment Strategy

### Automated Deployment
The CD pipeline (`.github/workflows/cd.yml`) runs on every push to `main` and:

1. **Cross-compiles** the Rust binary for `aarch64-unknown-linux-musl` target using the `cross` tool
2. **Strips** the binary to minimize size
3. **Verifies** binary size does not exceed 60 MB
4. **Deploys** via SSH:
   - Backs up the current binary as `knowledge-vault.backup`
   - Uploads the new binary to the configured path
   - Makes the binary executable
   - Restarts the systemd service
5. **Health check**: Waits up to 60 seconds for the service to become active

### Configuration

The deployment requires the following GitHub Actions secrets:

| Secret | Description | Example |
|--------|-------------|---------|
| `DEPLOY_SSH_KEY` | Private SSH key for authentication | (PEM format, SSH key without passphrase) |
| `DEPLOY_HOST` | Oracle Cloud VM IP or hostname | `123.45.67.89` |
| `DEPLOY_USER` | SSH user on the target VM | `ubuntu` |
| `DEPLOY_PATH` | Directory path on the VM (without trailing slash) | `/usr/local/bin` |

**To set secrets in GitHub:**
```bash
gh secret set DEPLOY_SSH_KEY < ~/.ssh/knowledge-vault-deploy
gh secret set DEPLOY_HOST -b "123.45.67.89"
gh secret set DEPLOY_USER -b "ubuntu"
gh secret set DEPLOY_PATH -b "/usr/local/bin"
```

## Rollback Procedure

### Manual Rollback (Recommended for Production Issues)

If the deployed binary causes issues, manually roll back using SSH:

```bash
ssh ubuntu@<DEPLOY_HOST> << 'EOF'
# Verify backup exists
ls -la /usr/local/bin/knowledge-vault.backup

# Restore previous binary
cp /usr/local/bin/knowledge-vault.backup /usr/local/bin/knowledge-vault

# Restart service
systemctl restart knowledge-vault

# Verify service is active
systemctl is-active knowledge-vault
EOF
```

### Automatic Rollback (via GitHub Actions)

If the health check fails, the deployment job fails and the previous binary remains in place. No automatic rollback action is taken — you must manually restore from the backup.

### Verification After Rollback

After rolling back, verify the service is operational:

```bash
ssh ubuntu@<DEPLOY_HOST> << 'EOF'
# Check service status
systemctl status knowledge-vault

# Test the health endpoint
curl http://localhost:8080/health

# Check recent logs
journalctl -u knowledge-vault -n 50 --no-pager
EOF
```

## Pre-Deployment Checklist

Before deploying to production, ensure:

1. All CI checks pass (lint, tests, security audit)
2. The binary builds successfully for `aarch64-unknown-linux-musl`
3. Binary size is under 60 MB
4. Oracle Cloud VM is reachable via SSH with the configured credentials
5. The systemd service file exists and is configured correctly (see below)

## Service Configuration (systemd)

The VM must have a systemd service file at `/etc/systemd/system/knowledge-vault.service`:

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
StandardOutput=journal
StandardError=journal
SyslogIdentifier=knowledge-vault

# Environment variables
Environment="KV_DATA_DIR=/var/lib/knowledge-vault"
Environment="KV_PORT=8080"
Environment="KV_JWT_SECRET=<your-secret>"
Environment="KV_GEMINI_API_KEY=<your-api-key>"

[Install]
WantedBy=multi-user.target
```

Enable and start the service:

```bash
sudo systemctl daemon-reload
sudo systemctl enable knowledge-vault
sudo systemctl start knowledge-vault
```

## Health Check Details

The CD pipeline runs a health check that:

1. Waits for the systemd service to report `active (running)`
2. Times out after 60 seconds (30 retries × 2 seconds)
3. Fails the deployment job if the service does not become active

The systemd service can report `active` but the HTTP endpoint may still be starting. The health check validates service state, not HTTP endpoint availability.

## Monitoring and Logs

### View recent logs

```bash
ssh ubuntu@<DEPLOY_HOST> journalctl -u knowledge-vault -n 100 -f
```

### Check service status

```bash
ssh ubuntu@<DEPLOY_HOST> systemctl status knowledge-vault
```

### Monitor via HTTP

```bash
curl http://<DEPLOY_HOST>:8080/health
```

Expected response:
```json
{
  "status": "ok",
  "version": "...",
  "db": "connected"
}
```

## Known Limitations

- **No blue-green deployment**: The current strategy is simple binary replacement with a single VM. No zero-downtime deployment.
- **No automatic metrics**: Deployment success is measured only by systemd service status, not by application-level metrics.
- **No canary deployment**: All traffic immediately switches to the new binary on restart.
- **Backup retention**: Only the immediate previous binary is backed up. Older versions are not retained.

## Future Improvements

For production-grade deployment maturity:

1. Add Prometheus metrics export to verify application health (not just systemd status)
2. Implement canary deployment with 2 VMs and load balancer
3. Add automated log aggregation and alerting
4. Retain 5+ binary versions for easier rollback
5. Add deployment notifications to Slack/PagerDuty
