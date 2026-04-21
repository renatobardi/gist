# Changelog

All notable changes to Knowledge Vault will be documented in this file.

## [Unreleased]

### Added

#### Personal Access Tokens (2026-04-20)
- Personal Access Token (PAT) generation and management: `POST /api/tokens`, `GET /api/tokens`, `DELETE /api/tokens/{id}`
- PAT format: `ens_` prefix followed by 32 random bytes (base64url-encoded)
- PAT authentication: PATs work identically to JWT tokens in Authorization header
- PAT storage: Hashed with Argon2id (OWASP recommended parameters: m=19456, t=2, p=1) before storage
- PAT security: Raw token shown only once at creation; never returned in list operations
- Token metadata: Creation timestamp, human-readable name, unique ID for revocation
- Revocation support: Immediate rejection of revoked tokens (401 Unauthorized)
- Full test coverage: Create, list, revoke, cross-auth (PAT-to-PAT), revocation validation
- Documentation: API reference, user guide with code examples (Bash, Python, Node.js, GitHub Actions), security best practices, token rotation patterns

#### Email/Password Authentication (2026-04-20)
- Email/password login endpoint: `POST /auth/login`
- JWT session tokens (HS256, 24-hour expiry) with HttpOnly/Secure/SameSite-Strict cookies
- First-run admin account setup: `POST /setup`
- Rate limiting: maximum 3 failed login attempts per email per 5-minute window (returns 429)
- Password validation: minimum 12 characters
- Password hashing: Argon2id (OWASP 2026 parameters: m=65536, t=3, p=1)
- Login attempt tracking in SurrealDB for rate limiting
- Security headers: X-Content-Type-Options, X-Frame-Options, CSP, Referrer-Policy
- Environment variable support for JWT secret: `KV_JWT_SECRET`
- Integration tests covering happy path, rate limiting, and error cases

#### Design Phase Completion (2026-04-20)
- Complete UX design system following BMW design principles
- 26 design tokens (colors, typography, spacing)
- Component specifications: Button, Card, Form Field, Status Badge with WCAG 2.1 AA compliance
- User flows and information architecture for 6 pages (setup, login, library, detail, graph, concept-detail)
- Interactive wireframes with Mermaid diagrams for all pages
- Interaction patterns documentation (validation, loading, WebSocket updates, graph interactions)
- Design artifacts published in `_bmad/docs/design/` directory

### Documentation
- Added Design section to README with references to all design artifacts
- Added API Reference section to README with authentication endpoints and security details
- Documented environment variable requirements: KV_JWT_SECRET, KV_GEMINI_API_KEY, KV_DATA_DIR, KV_PORT
- Documented login endpoint request/response formats, rate limiting behavior, and password requirements
- Documented security headers and cookie attributes (HttpOnly, Secure, SameSite)
- Updated architecture.md with design system & components section (section 8)
- Published design tokens and component specifications for implementation reference

## Legend

- **Added** for new features.
- **Changed** for changes in existing functionality.
- **Deprecated** for soon-to-be removed features.
- **Removed** for now removed features.
- **Fixed** for any bug fixes.
- **Security** in case of vulnerabilities.
