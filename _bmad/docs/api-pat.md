# Personal Access Token (PAT) API

Personal Access Tokens provide an alternative authentication method for programmatic access to the Knowledge Vault API. PATs are ideal for service-to-service communication, CI/CD automation, and batch operations.

## Token Format

PATs use a fixed `ens_` prefix followed by 32 cryptographically random bytes encoded in base64url format:

```
ens_<32-bytes-base64url>
```

Example: `ens_X8f2mK9qR7nZpYwL3tBvCuEjHaGsMdNiOlPkQeWAbcD`

## Storage & Security

- **Generation**: Random bytes generated using OS-level entropy (OsRng)
- **Hashing**: Argon2id password hash with random salt (OWASP recommended parameters: m=19456, t=2, p=1)
- **Display**: Raw token shown **only once** at creation time; never returned in list operations
- **Storage**: Only the Argon2id hash is persisted in SurrealDB
- **Verification**: Runtime comparison using Argon2id verification

## Endpoints

### Create a Personal Access Token

```
POST /api/tokens
Authorization: Bearer <JWT or PAT>
Content-Type: application/json

{
  "name": "ci-deployment-token"
}
```

**Request Parameters**

| Parameter | Type | Required | Constraints | Description |
|-----------|------|----------|-------------|-------------|
| `name` | string | Yes | 1-256 characters, non-empty after trim | Human-readable name for the token (e.g., "GitHub Actions", "Backup Script") |

**Response: 201 Created**

```json
{
  "token_id": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
  "token": "ens_X8f2mK9qR7nZpYwL3tBvCuEjHaGsMdNiOlPkQeWAbcD",
  "name": "ci-deployment-token"
}
```

⚠️ **Important**: The raw token is shown only once. Store it securely (password manager, secrets service, etc.). If lost, create a new token.

**Response: 422 Unprocessable Entity**

```json
{
  "error": "invalid_input",
  "message": "Token name must not be empty",
  "status": 422
}
```

Valid error messages:
- "Token name must not be empty" (whitespace-only names)
- "Token name must not exceed 256 characters"

**Response: 401 Unauthorized**

Returned when:
- No Authorization header provided
- Authorization header format is invalid
- JWT is expired or invalid
- PAT is revoked or invalid

---

### List Personal Access Tokens

```
GET /api/tokens
Authorization: Bearer <JWT or PAT>
```

**Response: 200 OK**

```json
[
  {
    "token_id": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
    "name": "ci-deployment-token",
    "created_at": "2026-04-20T18:42:47Z"
  },
  {
    "token_id": "yyyyyyyy-yyyy-yyyy-yyyy-yyyyyyyyyyyy",
    "name": "backup-script",
    "created_at": "2026-04-21T10:15:30Z"
  }
]
```

**Response Format**

| Field | Type | Description |
|-------|------|-------------|
| `token_id` | string (UUID) | Unique identifier for the token (used to revoke) |
| `name` | string | Human-readable name assigned at creation |
| `created_at` | string (RFC3339) | ISO 8601 timestamp when the token was created |

⚠️ **Note**: The raw token value is never returned in list operations. This endpoint only shows metadata.

**Response: 401 Unauthorized**

Returned when authentication fails (see Create endpoint).

---

### Revoke/Delete a Personal Access Token

```
DELETE /api/tokens/{token_id}
Authorization: Bearer <JWT or PAT>
```

**Path Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `token_id` | string (UUID) | The token ID (from list or creation response) |

**Response: 204 No Content**

Token successfully revoked. Subsequent uses of this token will be rejected with 401.

**Response: 404 Not Found**

```json
{
  "error": "not_found",
  "message": "Token not found",
  "status": 404
}
```

Returned when:
- Token ID does not exist
- Token belongs to a different user

**Response: 401 Unauthorized**

Returned when authentication fails (see Create endpoint).

---

## Authentication with PATs

PATs are used identically to JWT tokens — pass them as Bearer tokens in the Authorization header:

```bash
curl -H "Authorization: Bearer ens_X8f2mK9qR7nZpYwL3tBvCuEjHaGsMdNiOlPkQeWAbcD" \
     https://vault.example.com/api/tokens
```

### Using PAT to Create Another PAT

A PAT can authenticate requests that create additional PATs:

```bash
# Create initial token (with JWT)
curl -X POST https://vault.example.com/api/tokens \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name": "initial-token"}'

# Use that token to create a second token
curl -X POST https://vault.example.com/api/tokens \
  -H "Authorization: Bearer $PAT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name": "second-token"}'
```

### Revoking a PAT Immediately Rejects It

Once revoked, a PAT cannot be used for any subsequent request:

```bash
# Create and then revoke a token
TOKEN_ID=$(curl -X POST https://vault.example.com/api/tokens \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -d '{"name": "temp"}' | jq -r '.token_id')

curl -X DELETE https://vault.example.com/api/tokens/$TOKEN_ID \
  -H "Authorization: Bearer $JWT_TOKEN"

# Any subsequent request with that token is rejected
curl https://vault.example.com/api/tokens \
  -H "Authorization: Bearer $REVOKED_PAT"
# → 401 Unauthorized
```

---

## Error Responses

All error responses follow the format:

```json
{
  "error": "<error_code>",
  "message": "<human_readable_message>",
  "status": <http_status_code>
}
```

Common error codes:

| Code | Status | Meaning |
|------|--------|---------|
| `invalid_input` | 422 | Validation error (empty name, name too long) |
| `not_found` | 404 | Token ID does not exist |
| `internal_error` | 500 | Server error (database, etc.) |
| `unauthorized` | 401 | Missing/invalid authentication |
