# Changelog

All notable changes to Knowledge Vault will be documented in this file.

## [Unreleased]

### Added

#### MVP 2 Schema Migration (Phase 0) (2026-04-24)
- Extended `work` table with metadata and reading tracking fields:
  - `progress_pct` (option<float>): Reading progress percentage (0-100)
  - `last_action` (option<datetime>): Timestamp of last user interaction
  - `reading_status` (option<string>): Current reading state (e.g., "not_started", "in_progress", "completed")
  - `cover_image_url` (option<string>): URL to work cover image
  - `page_count` (option<int>): Total pages in work
  - `publisher` (option<string>): Publishing company
  - `average_rating` (option<float>): Community rating (0-5)
  - `preview_link` (option<string>): Link to preview content
- Extended `users` table with personalization fields:
  - `display_name` (option<string>): User's public display name
  - `preferences` (option<object>): Unstructured user preferences (extensible for future use)
- All fields are optional and added via safe, idempotent `DEFINE FIELD IF NOT EXISTS` syntax
- No data migration required; existing records remain unchanged with null values for new fields
- Added `work_created_at` index on `work.created_at` for query performance
- PR: [https://github.com/renatobardi/gist/pull/52](https://github.com/renatobardi/gist/pull/52)

### Documentation
- Updated schema documentation with new `work` and `users` table fields
- Updated architecture data model section with extended entity definitions

#### Worker Pipeline: Progress Tracking & Google Books Integration (2026-04-24)
- **4-stage progress pipeline** with real-time progress tracking (S01-02):
  - Stage 0 (0%): Fetch metadata from Open Library
  - Stage 1 (25%): Enrich with Google Books (optional, non-fatal)
  - Stage 2 (50%): Extract concepts with Gemini API
  - Stage 3 (75%): Write results to knowledge graph
  - Stage 4 (100%): Complete
- **Progress persistence** to work record: `progress_pct` and `last_action` fields updated at each checkpoint
- **WebSocket broadcasts** for real-time progress updates: clients receive `{"type": "work_progress", "work_id": "...", "progress_pct": N, "last_action": "..."}` messages
- **Google Books adapter integration** for ISBN-based submissions:
  - Conditional execution: only runs for ISBN submissions when `KV_GOOGLE_BOOKS_API_KEY` is set
  - Non-fatal error handling: transient and permanent errors in Google Books do not fail the pipeline
  - Metadata enrichment: persists `cover_image_url`, `page_count`, `publisher`, `average_rating`, `preview_link`
  - Fallback handling: Open Library covers API provides fallback cover URLs when Google Books returns no image
- **Error classification** refined: transient errors (timeouts, rate limits) retry with exponential backoff; permanent errors (invalid ISBN, schema violations) fail immediately
- **Monitoring via REST**: clients can poll `GET /api/works/{id}` to retrieve current `progress_pct` and `last_action`
- Full test coverage for progress emission, Google Books error scenarios, and WebSocket broadcasts
- PR: [https://github.com/renatobardi/gist/pull/TBD](https://github.com/renatobardi/gist/pull/TBD)

### Documentation
- Added comprehensive [Worker Pipeline Guide](_bmad/docs/worker-pipeline.md) with architecture, progress tracking, Google Books integration, error handling, and development guide
- Updated [Works API documentation](_bmad/docs/api-works.md) with progress monitoring sections (WebSocket events and REST polling)
- Updated [README.md](_bmad/docs/) with links to worker pipeline documentation

#### Health Check Endpoint (2026-04-21)
- `GET /health` endpoint for service and database connectivity monitoring
- HTTP 200 with `{status: "ok", version: "...", db: "connected"}` when database is reachable
- HTTP 503 with `{status: "degraded", db: "disconnected"}` when database is unavailable
- Supports deployment health checks and load balancer monitoring
- No authentication required
- Includes binary version from `CARGO_PKG_VERSION`
- Database connectivity verified with non-blocking `RETURN 1` query
- PR: [https://github.com/renatobardi/gist/pull/35](https://github.com/renatobardi/gist/pull/35)

### Documentation
- Added Health Check Endpoint section to README API Reference

#### Transactional Graph Write Persistence (2026-04-21)
- New `GraphWriteRepo` port trait for atomic graph write transactions.
- Implementation using SurrealDB's `BEGIN TRANSACTION` / `COMMIT` to ensure all-or-nothing semantics for graph population (concept upserts, edge creations, insight persistence, and work status updates).
- Guarantees data consistency by rolling back the entire operation if any step fails.
- PR: [https://github.com/renatobardi/gist/pull/31](https://github.com/renatobardi/gist/pull/31)

#### Submit Book by ISBN (2026-04-21)
- `POST /api/works` endpoint: accepts ISBN-10 or ISBN-13 for book submission
- Automatic metadata fetching from Open Library API
- Duplicate ISBN detection (returns 409 Conflict)
- Asynchronous processing of book data for knowledge graph generation
- NATS `discovery.requested` event published upon successful ingestion

### Documentation
- Added `api-works.md` for `POST /api/works` endpoint with request/response details
- Updated `architecture.md` API Design section to include `works` endpoints
- Updated `README.md` with link to Works API documentation

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

### Added

#### Continuous Deployment (CD) Pipeline (2026-04-21)
- Implemented GitHub Actions CD workflow (`.github/workflows/cd.yml`) for `knowledge-vault`.
- Cross-compilation to `aarch64-unknown-linux-musl` target for Oracle Cloud ARM64 instances.
- SSH deployment mechanism including backup, installation, and service restart (`systemctl`).
- Post-deployment health check with exponential backoff (30 retries × 2s).
- Documented rollback procedure.
- Required secrets for CD workflow (DEPLOY_HOST, DEPLOY_USER, DEPLOY_PATH, DEPLOY_SSH_KEY) documented in README.

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
