# Personal Access Token User Guide

Learn how to generate, manage, and use Personal Access Tokens (PATs) for programmatic access to Knowledge Vault.

## When to Use PATs

Use PATs for:
- **Automated scripts and batch operations** — CLI tools, backup scripts, data migrations
- **CI/CD pipelines** — GitHub Actions, GitLab CI, Jenkins
- **Service-to-service communication** — Microservices, webhooks, integrations
- **Long-running processes** — Batch jobs, scheduled tasks

Don't use PATs for:
- Interactive user sessions (use JWT login instead)
- One-off manual API calls (use JWT login instead)

## Generating a PAT

### Step 1: Log In to Knowledge Vault

Log in using your email and password:

```bash
curl -X POST https://vault.example.com/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "email": "user@example.com",
    "password": "your-12+-character-password"
  }'
```

Response:

```json
{
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
}
```

Save the JWT token for the next step (or extract it from the Set-Cookie header).

### Step 2: Create a PAT

Use your JWT to create a PAT:

```bash
curl -X POST https://vault.example.com/api/tokens \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "github-actions-deploy"
  }'
```

Response:

```json
{
  "token_id": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
  "token": "ens_<32-bytes-base64url>",
  "name": "github-actions-deploy"
}
```

⚠️ **Critical**: The `token` value is shown only once. Copy it immediately and store it securely.

### Step 3: Store the Token Securely

**Do NOT:**
- Store in version control (git)
- Store in plaintext configuration files
- Commit to a repository
- Log it in application output

**Do:**
- Store in a password manager (1Password, Bitwarden, LastPass)
- Store in a CI/CD platform's secret management (GitHub Secrets, GitLab CI Variables)
- Store in environment variables on trusted machines only
- Use OS-level secret storage (macOS Keychain, Windows Credential Manager, Linux libsecret)

## Using a PAT

### In Bash/Shell Scripts

```bash
#!/bin/bash

# Load the PAT from environment (never hardcode tokens in scripts)
PAT="${VAULT_PAT:?VAULT_PAT env var is required}"
VAULT_URL="https://vault.example.com"

# Make API calls with the PAT
curl -X GET "$VAULT_URL/api/tokens" \
  -H "Authorization: Bearer $PAT"
```

### In GitHub Actions

Create a secret in your GitHub repository:

1. Go to **Settings** → **Secrets and variables** → **Actions**
2. Click **New repository secret**
3. Name: `KNOWLEDGE_VAULT_PAT`
4. Value: (paste the PAT from step 2)

Use it in your workflow:

```yaml
name: Sync to Knowledge Vault
on: [push]

jobs:
  sync:
    runs-on: ubuntu-latest
    steps:
      - name: List active tokens
        run: |
          curl -X GET https://vault.example.com/api/tokens \
            -H "Authorization: Bearer ${{ secrets.KNOWLEDGE_VAULT_PAT }}"
```

### In Python

```python
import requests
import os

PAT = os.environ.get("KNOWLEDGE_VAULT_PAT")
VAULT_URL = "https://vault.example.com"

# List active tokens
response = requests.get(
    f"{VAULT_URL}/api/tokens",
    headers={"Authorization": f"Bearer {PAT}"}
)

if response.status_code == 200:
    tokens = response.json()
    for t in tokens:
        print(f"{t['name']} (ID: {t['token_id']}, created: {t['created_at']})")
else:
    print(f"Error: {response.status_code}")
```

### In JavaScript/Node.js

```javascript
const PAT = process.env.KNOWLEDGE_VAULT_PAT;
const VAULT_URL = "https://vault.example.com";

// List active tokens
const response = await fetch(`${VAULT_URL}/api/tokens`, {
  headers: {
    "Authorization": `Bearer ${PAT}`
  }
});

if (response.ok) {
  const tokens = await response.json();
  tokens.forEach(t => console.log(`${t.name} (ID: ${t.token_id})`));
} else {
  console.error(`Error: ${response.status}`);
}
```

## Managing PATs

### List Your PATs

```bash
curl -X GET https://vault.example.com/api/tokens \
  -H "Authorization: Bearer $JWT_TOKEN"
```

Response:

```json
[
  {
    "token_id": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
    "name": "github-actions-deploy",
    "created_at": "2026-04-20T18:42:47Z"
  },
  {
    "token_id": "yyyyyyyy-yyyy-yyyy-yyyy-yyyyyyyyyyyy",
    "name": "backup-script",
    "created_at": "2026-04-21T10:15:30Z"
  }
]
```

### Revoke a PAT

```bash
curl -X DELETE https://vault.example.com/api/tokens/{token_id} \
  -H "Authorization: Bearer $JWT_TOKEN"
```

Returns `204 No Content` on success.

**After revoking**, the PAT can no longer be used for authentication. Any requests with a revoked token will be rejected with `401 Unauthorized`.

## Security Best Practices

### Token Rotation

Rotate PATs periodically to minimize exposure window:

1. Create a new PAT with a new name
2. Update your application/script to use the new PAT
3. Test that everything works
4. Revoke the old PAT
5. Delete the old PAT from secret storage

Example rotation script:

```bash
#!/bin/bash

JWT_TOKEN="$1"
OLD_TOKEN_ID="$2"
VAULT_URL="https://vault.example.com"

# Create replacement token
NEW_TOKEN=$(curl -s -X POST "$VAULT_URL/api/tokens" \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name": "rotated-token-'$(date +%s)'"}' \
  | jq -r '.token')

echo "New token: $NEW_TOKEN"
echo "Update your configuration, then run:"
echo "curl -X DELETE $VAULT_URL/api/tokens/$OLD_TOKEN_ID \\"
echo "  -H \"Authorization: Bearer $JWT_TOKEN\""
```

### Scope Limitation

- Each PAT has a single human-readable name
- All PATs have the same permissions as the user who created them
- Create separate tokens for separate purposes (one for CI/CD, one for backups, etc.)
- This helps identify which process is using which token

### Monitoring for Compromise

Regularly check your active tokens:

```bash
# List all PATs and their creation dates
curl -X GET https://vault.example.com/api/tokens \
  -H "Authorization: Bearer $JWT_TOKEN" | jq '.[] | {name, created_at}'
```

If you see unexpected tokens:
- Revoke them immediately
- Change your Knowledge Vault password
- Review recent login activity logs (if available)

### Storage in Secrets Managers

Recommended services:
- **Local**: 1Password, Bitwarden (with their CLI)
- **Cloud**: AWS Secrets Manager, Azure Key Vault, Google Secret Manager
- **CI/CD**: GitHub Secrets, GitLab CI Variables, HashiCorp Vault

Example using AWS Secrets Manager:

```bash
# Store the PAT
aws secretsmanager create-secret \
  --name knowledge-vault/github-actions \
  --secret-string "$VAULT_PAT"

# Retrieve in a script
PAT=$(aws secretsmanager get-secret-value \
  --secret-id knowledge-vault/github-actions \
  --query SecretString --output text)

curl -H "Authorization: Bearer $PAT" https://vault.example.com/api/tokens
```

## Troubleshooting

### "Token not found" (404)

**Cause**: The token ID doesn't exist or belongs to a different user.

**Solution**: Use `GET /api/tokens` to list valid token IDs and verify you have the correct ID.

### "Unauthorized" (401)

**Causes**:
- Token is revoked
- Token is malformed
- Token expired (for JWTs, not PATs)
- Authentication header is missing or incorrectly formatted

**Solution**:
1. Verify the token value is correct (copy-paste from secret manager)
2. Check the Authorization header format: `Authorization: Bearer <token>`
3. If the token was created long ago, list tokens to verify it still exists
4. Create a new token if necessary

### "Token name must not be empty" (422)

**Cause**: The name field is empty or contains only whitespace.

**Solution**: Provide a non-empty name for the token (1-256 characters).

### "Token name must not exceed 256 characters" (422)

**Cause**: The name is longer than 256 characters.

**Solution**: Use a shorter, descriptive name.

## Reference

- [PAT API Documentation](./api-pat.md)
- [Authentication Details](../README.md#-authentication) in main README
