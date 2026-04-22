#!/usr/bin/env bash
# One-time Caddy setup for HTTPS at gist.oute.pro on Ubuntu/Debian ARM64.
# Run this once on the Oracle Cloud VM as a user with sudo privileges.
set -euo pipefail

# Install Caddy from the official Debian/Ubuntu package repository
sudo apt-get install -y debian-keyring debian-archive-keyring apt-transport-https curl
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
    | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
    | sudo tee /etc/apt/sources.list.d/caddy-stable.list
sudo apt-get update
sudo apt-get install -y caddy

# Deploy the Caddyfile
sudo cp "$(dirname "$0")/Caddyfile" /etc/caddy/Caddyfile

# Enable and start Caddy (it manages its own systemd unit)
sudo systemctl daemon-reload
sudo systemctl enable caddy
sudo systemctl restart caddy

echo "Caddy installed and running. Verify with:"
echo "  sudo systemctl status caddy"
echo "  curl -I https://gist.oute.pro/health"
