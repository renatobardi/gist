# Architecture Document: Knowledge Vault MVP 2

**Version:** 1.0
**Date:** 2026-04-24
**Architect Agent:** BMAD Pipeline — Architecture Phase
**Input:** `_bmad/docs/product-brief-mvp2.md`, `_bmad/docs/prd.md` (MVP 1), existing codebase
**Issue:** REN-141

---

## 1. Tech Stack Selection

### Core Stack (Unchanged from MVP 1)

| Component | Choice | Version |
|-----------|--------|---------|
| Language | Rust | 1.78+ stable |
| HTTP framework | Axum | 0.8+ |
| Async runtime | Tokio | 1.x |
| Frontend | Leptos | 0.7+ (SSR + WASM) |
| Database | SurrealDB (SurrealKV storage) | 2.x crate / SurrealKV embedded |
| Messaging | NATS JetStream | async-nats 0.47+ |
| LLM | Gemini Flash | via reqwest (plain HTTP) |
| Auth | JWT (HS256) + Argon2id PATs | jsonwebtoken 9 / argon2 0.5 |

The single-binary `aarch64-unknown-linux-musl` constraint is preserved. No new runtime dependencies are added.

### MVP 2 Additions

| Component | Choice | Justification |
|-----------|--------|---------------|
| Google Books API client | `reqwest` (existing) | Same HTTP client already in use for Gemini and Open Library. No additional crate. A new adapter module is added: `src/adapters/google_books/`. |
| Cover fallback | Open Library Covers API | Static URL pattern (`https://covers.openlibrary.org/b/isbn/{isbn}-L.jpg`). Zero API key. Already available from OL integration context. |

**Google Books API key:** Optional. `KV_GOOGLE_BOOKS_API_KEY` env var. If absent, the pipeline skips the Google Books step and advances to 50% without a metadata fetch. Cover fallback uses Open Library Covers API when Google Books is unavailable or returns no cover.

**No new dependencies added.** The `reqwest` client is already present in `Cargo.toml`. All new functionality is implemented via new adapter structs implementing existing or new port traits.

**Known trade-off:** Google Books `volumes` API keys have per-day quotas (10,000 units/day on free tier). For a personal tool with 3–10 submissions/week, this is irrelevant. If the key is absent or the API fails, the pipeline continues gracefully with Open Library data only.

---

## 2. System Architecture

### Component Overview

```
┌────────────────────────────────────────────────────────────┐
│                   knowledge-vault binary                   │
│                                                            │
│  ┌──────────────┐    ┌─────────────────────────────────┐   │
│  │  Axum HTTP   │    │         NATS Worker             │   │
│  │  Server      │    │         (WorkerService)         │   │
│  │              │    │                                 │   │
│  │  Handlers    │    │  OpenLib → GoogleBooks → Gemini │   │
│  │  (web/)      │    │  → GraphWrite                  │   │
│  │              │◄───│  emits progress via             │   │
│  │  WS endpoint │    │  Arc<WsBroadcaster>             │   │
│  └──────┬───────┘    └─────────────────────────────────┘   │
│         │                                                  │
│  ┌──────▼───────────────────────────────────────────────┐  │
│  │               AppState (Arc-shared)                  │  │
│  │  db · repos · ws_broadcaster · message_publisher     │  │
│  │  google_books_client · open_library_client           │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                            │
│  ┌────────────────┐  ┌────────────────┐                    │
│  │  SurrealDB     │  │  NATS Server   │                    │
│  │  (SurrealKV)   │  │  (embedded)    │                    │
│  └────────────────┘  └────────────────┘                    │
└────────────────────────────────────────────────────────────┘
```

### MVP 2 Structural Changes

**1. WorkerService gains two new dependencies:**
- `ws_broadcaster: Arc<WsBroadcaster>` — injected at startup, before the NATS consumer is spawned. In `main.rs`, `WsBroadcaster::new()` must be called before the NATS/worker block (currently it is called after — this ordering must change).
- `google_books: Option<Arc<dyn GoogleBooksPort>>` — `None` if `KV_GOOGLE_BOOKS_API_KEY` is unset.

**2. WsBroadcaster message schema is extended:**
```json
{
  "type": "work_progress",
  "work_id": "<uuid>",
  "status": "processing",
  "progress_pct": 25,
  "last_action": "Open Library fetch complete"
}
```
The existing `work_status` message type is preserved for backward compatibility; `work_progress` is the new type used by the worker pipeline.

**3. New HTTP routes:**
```
GET  /api/profile                → user profile
PATCH /api/profile               → update display_name / preferences
POST  /api/profile/email         → change email (requires current_password)
POST  /api/profile/password      → change password (requires current_password)
DELETE /api/works/{id}           → cascade-delete work
PATCH  /api/works/{id}           → update reading_status
GET    /api/library/domains      → distinct concept domains (for filter UI)
POST   /api/library/group        → ephemeral AI grouping
GET    /api/works?status=&domain=&sort=&order=&limit=&offset=  (extended)
```

**4. AppState additions:**
```rust
pub google_books_client: Option<Arc<dyn GoogleBooksPort>>,
```

### Critical User Journey: Book Submission with Progress (FR-07–11, US-05)

```
Browser POST /api/works
→ Handler publishes NATS message "discovery.requested"
→ Returns 202 Accepted {work_id}

Worker receives message:
  1. update_status("processing")
  2. update_progress(0,  "Starting...") + broadcast(0%,  "processing", "Starting...")
  3. OpenLib fetch
     update_progress(25, "Open Library fetch complete") + broadcast(25%)
  4. Google Books fetch (if key set)
     update_progress(50, "Google Books fetch complete") + broadcast(50%)
     OR advance to 50% with "Google Books skipped" if key absent
  5. Gemini extraction
     update_progress(75, "Concept extraction complete") + broadcast(75%)
  6. Atomic graph write (sets status=done, progress=100 in transaction)
     broadcast(100%, "done", "Processing complete")

Browser WebSocket /ws receives progress events → updates UI in real time
```

---

## 3. Data Model

### Schema Changes (All Additive — DEFINE IF NOT EXISTS)

**Work table — new fields:**

```sql
DEFINE FIELD IF NOT EXISTS progress_pct   ON work TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS last_action    ON work TYPE string DEFAULT '';
DEFINE FIELD IF NOT EXISTS reading_status ON work TYPE option<string>;

-- Google Books enrichment fields (populated by worker, optional)
DEFINE FIELD IF NOT EXISTS cover_image_url ON work TYPE option<string>;
DEFINE FIELD IF NOT EXISTS page_count      ON work TYPE option<int>;
DEFINE FIELD IF NOT EXISTS publisher       ON work TYPE option<string>;
DEFINE FIELD IF NOT EXISTS average_rating  ON work TYPE option<float>;
DEFINE FIELD IF NOT EXISTS preview_link    ON work TYPE option<string>;
```

**Rationale for placing Google Books fields on `work`:** These are book-level metadata (properties of the physical object), not knowledge-graph concepts. The `insight` table stores Gemini's extracted intelligence. `work` stores canonical book facts.

**Cover fallback logic:** `cover_image_url` is populated by the worker: first from Google Books (if available), then from `https://covers.openlibrary.org/b/isbn/{isbn}-L.jpg` (if isbn is set), else `null`.

**Users table — new fields:**

```sql
DEFINE FIELD IF NOT EXISTS display_name ON users TYPE option<string>;
DEFINE FIELD IF NOT EXISTS preferences  ON users TYPE option<object>;
```

`preferences` is a flexible object stored as JSON. Initial shape:
```json
{ "theme": "dark", "default_sort": "date_added", "default_sort_order": "desc" }
```

No schema enforcement at the SurrealDB layer — the application layer validates and sets defaults. This avoids needing a migration when preferences fields are added in MVP 3.

### Entity Relationship Diagram

```
erDiagram
    users {
        string id PK
        string email
        string password_hash
        string role
        string display_name "nullable"
        object preferences "nullable"
        datetime created_at
    }

    work {
        string id PK
        string title
        string author
        string isbn "nullable, unique"
        string open_library_id "nullable, unique"
        string status
        string error_msg "nullable"
        int progress_pct
        string last_action
        string reading_status "nullable: want_to_read|reading|finished"
        string cover_image_url "nullable"
        int page_count "nullable"
        string publisher "nullable"
        float average_rating "nullable"
        string preview_link "nullable"
        datetime created_at
        datetime updated_at
    }

    insight {
        string id PK
        string summary
        array key_points
        string raw_gemini_response
        datetime created_at
    }

    concept {
        string id PK
        string name
        string display_name
        string description
        string domain
        datetime created_at
    }

    work ||--o{ interpreta : "->interpreta->"
    interpreta ||--|| insight : ""
    insight ||--o{ menciona : "->menciona->"
    menciona {
        float relevance_weight
    }
    menciona ||--|| concept : ""
    concept ||--o{ relacionado_a : "->relacionado_a->"
    relacionado_a {
        string relation_type
        float strength
    }
    relacionado_a ||--|| concept : ""
```

### Migration Strategy

All changes use `DEFINE IF NOT EXISTS` statements appended to `SCHEMA_SQL` in `src/adapters/surreal/schema.rs`. No destructive operations. Existing `work` rows will have `NULL` values for new optional fields — this is the expected state until the worker re-processes them.

`progress_pct` defaults to `0` and `last_action` defaults to `''` for existing rows. These values are correct for already-done works (the `status = 'done'` field identifies their true state).

---

## 4. API Design

### API Style and Versioning

Unchanged from MVP 1: REST, JSON bodies, all authenticated endpoints require `Authorization: Bearer <jwt>` or session cookie. No API versioning prefix for MVP 2 (single-user, no external consumers).

### Authentication

Unchanged from MVP 1: `AuthenticatedUser` extractor (middleware) validates JWT or PAT from Bearer header or httponly session cookie.

### New Endpoints

#### User Profile (F1, FR-01–06)

**GET /api/profile**
```json
Response 200:
{
  "id": "uuid",
  "email": "user@example.com",
  "display_name": "Renato",
  "preferences": { "theme": "dark", "default_sort": "date_added", "default_sort_order": "desc" },
  "created_at": "2026-04-20T..."
}
```

**PATCH /api/profile** — update display_name and/or preferences (no password required)
```json
Request:  { "display_name": "Renato B.", "preferences": { "theme": "light" } }
Response 200: updated profile object
```

**POST /api/profile/email** — change email (requires current password)
```json
Request:  { "current_password": "...", "new_email": "new@example.com" }
Response 200: { "message": "Email updated" }
Response 400: { "error": "Invalid password" }
Response 409: { "error": "Email already in use" }
```

**POST /api/profile/password** — change password (requires current password)
```json
Request:  { "current_password": "...", "new_password": "..." }
Response 200: { "message": "Password updated" }
Response 400: { "error": "Invalid current password" | "Password too short" }
```

#### Extended Works List (F4, FR-17–23)

**GET /api/works** — extended with filter and sort params
```
?status=pending|processing|done|failed   (server-side filter)
?domain=<string>                          (server-side graph traversal filter)
?sort=date_added|title|author|status|reading_status  (server-side ORDER BY)
?order=asc|desc                           (default: desc for date_added, asc otherwise)
?limit=<int>                              (default: 50, max: 200)
?offset=<int>                             (default: 0)
```

Domain filter SurrealQL (when `?domain=<value>` is provided):
```sql
SELECT * FROM work
WHERE ->interpreta->insight->menciona->concept.domain CONTAINS $domain
  AND ($status IS NONE OR status = $status)
ORDER BY $sort $order
LIMIT $limit START $offset
```

**GET /api/library/domains** — distinct domains for filter UI dropdown
```json
Response 200: { "domains": ["Philosophy", "Computer Science", "Economics"] }
```
SurrealQL: `SELECT DISTINCT domain FROM concept ORDER BY domain ASC`

#### Delete Work (F3, FR-12–16)

**DELETE /api/works/{id}**
```
Response 204: No Content (success)
Response 404: { "error": "Not found" }
```

Implementation: Single SurrealDB transaction:
```sql
BEGIN TRANSACTION;
LET $insight_ids = (SELECT ->interpreta->insight.id AS id FROM type::thing('work', $id))[0].id;
DELETE type::thing('work', $id)->interpreta;
FOR $iid IN $insight_ids {
  DELETE type::thing('insight', $iid)->menciona;
  DELETE type::thing('insight', $iid);
};
DELETE type::thing('work', $id);
COMMIT TRANSACTION;
```
Concept nodes are **not** deleted. `relacionado_a` edges between concepts are **not** deleted (they represent knowledge relationships independent of any single work).

#### Reading Status Update (S1, FR-33–36)

**PATCH /api/works/{id}**
```json
Request:  { "reading_status": "reading" }
Response 200: updated work object
Response 400: { "error": "Invalid reading_status" }  (not one of the allowed values)
```
Allowed values: `"want_to_read"`, `"reading"`, `"finished"`, `null` (unset).

#### AI Grouping (F4, FR-21–23)

**POST /api/library/group** — ephemeral AI clustering
```json
Request:  {} (empty, no body required)
Response 200:
{
  "groups": [
    { "label": "Behavioral Economics", "work_ids": ["uuid1", "uuid2"] },
    { "label": "System Design Patterns", "work_ids": ["uuid3", "uuid4", "uuid5"] }
  ]
}
Response 400: { "error": "At least 5 done books required" }
```

Precondition check: COUNT of `work WHERE status = 'done'` >= 5.

Prompt pattern: Send Gemini the list of done works (id, title, author, top 3 concepts each) and request JSON cluster labels. Response is **not persisted**; returned directly to caller.

### Error Response Format (Unchanged)

```json
{ "error": "<human-readable message>" }
```

Status codes: 200/201/204 success, 400 validation, 401 unauthorized, 404 not found, 409 conflict, 422 unprocessable, 500 internal.

---

## 5. Cross-Cutting Concerns

### Authentication & Authorization

Unchanged. All new endpoints (`/api/profile`, `DELETE /api/works/{id}`, `PATCH /api/works/{id}`, `POST /api/library/group`) require authentication via the existing `AuthenticatedUser` extractor.

Email/password change endpoints perform an additional in-handler password verification step (call `argon2::verify` against current hash) before executing the change.

### Progress Persistence vs. WebSocket

Both mechanisms are used:
- **DB persistence** (`progress_pct`, `last_action` on `work`): Durable. The UI can show correct progress even if the WebSocket connection dropped and reconnected, by fetching `GET /api/works/{id}` on reconnect.
- **WebSocket broadcast** (`WsBroadcaster`): Real-time delivery to connected clients. Fire-and-forget; a `let _ = tx.send(...)` pattern is appropriate (existing behavior).

### Observability

`tracing::info!` spans added for each pipeline stage:
```rust
info!(work_id, progress_pct, last_action, "pipeline stage complete");
```

No new infrastructure required.

### Security

- Reading status values validated in handler before DB write (`["want_to_read", "reading", "finished"]`).
- Email change: normalized to lowercase, validated against `email_address` crate before persistence.
- Password change: minimum 12 characters enforced via existing `validate_password()`.
- Delete endpoint: operates only on the authenticated user's data (single-user system; any authenticated user can delete any work — acceptable for personal use).
- `POST /api/library/group`: the Gemini prompt is constructed server-side from DB data only, never from user input. No prompt injection surface.

### Testing Strategy

| Layer | What | Where |
|-------|------|-------|
| Unit | Domain logic: reading_status validation, delete cascade helper, progress_pct transitions | `src/domain/` |
| Unit | WorkerService: progress broadcast calls at each pipeline step | `src/app/worker.rs` unit tests |
| Integration | SurrealDB delete cascade: verify no orphan edges remain | `tests/integration/` |
| Integration | Google Books adapter: mock HTTP server responses | `tests/integration/` |
| Integration | Profile update: email/password change with correct + incorrect current_password | `tests/integration/` |
| E2E (manual) | Library filter/sort: verify SurrealQL domain filter returns correct works | Manual |
| E2E (manual) | AI grouping: verify Gemini call fires, response shape correct, nothing persisted | Manual |

---

## 6. Project Structure

MVP 2 additions to the existing structure (new files marked with `[NEW]`):

```
knowledge-vault/src/
├── domain/
│   ├── work.rs                    # Add ReadingStatus enum + validation fn [EXTEND]
│   └── user.rs                    # Add UserPreferences struct [EXTEND]
├── ports/
│   ├── repository.rs              # Add WorkRepo::delete_work_cascade, WorkRepo::update_progress,
│   │                              #   WorkRepo::update_reading_status, UserRepo::find_by_id,
│   │                              #   UserRepo::update_profile [EXTEND]
│   └── external.rs                # Add GoogleBooksPort trait + GoogleBooksMetadata struct [EXTEND]
├── adapters/
│   ├── surreal/
│   │   ├── schema.rs              # Append new DEFINE IF NOT EXISTS fields [EXTEND]
│   │   ├── work_repo.rs           # Implement delete_work_cascade, update_progress,
│   │   │                          #   update_reading_status, list_works_filtered [EXTEND]
│   │   └── user_repo.rs           # Implement find_by_id, update_profile [EXTEND]
│   └── google_books/
│       └── mod.rs                 # GoogleBooksClient implementing GoogleBooksPort [NEW]
├── app/
│   └── worker.rs                  # Add ws_broadcaster + google_books deps; extend run_pipeline
│                                  #   with 4-stage progress + Google Books step [EXTEND]
└── web/
    ├── state.rs                   # Add google_books_client field [EXTEND]
    ├── router.rs                  # Register new routes [EXTEND]
    ├── ws_broadcaster.rs          # Add broadcast_progress method [EXTEND]
    └── handlers/
        ├── profile.rs             # GET/PATCH /api/profile, POST /api/profile/email|password [NEW]
        ├── works.rs               # Add DELETE handler + PATCH reading_status handler [EXTEND]
        └── library.rs             # Add domain filter + sort params; add group endpoint [EXTEND]
```

### Module Boundary Rules

- `domain/` has zero I/O dependencies. `ReadingStatus` validation lives here.
- `ports/` defines traits only — no implementations.
- `adapters/` implement ports; each adapter is replaceable (e.g., swap Google Books for ISBNdb later by implementing `GoogleBooksPort`).
- `app/worker.rs` may hold `Arc<WsBroadcaster>` directly (not behind a port trait) — the broadcaster is an infrastructure primitive, not an external API.

---

## 7. Implementation Constraints

| Constraint | Rationale |
|-----------|-----------|
| Single `aarch64-unknown-linux-musl` binary | Preserved. No new dynamic libraries. `reqwest` already uses `rustls-tls`. |
| Additive SurrealDB schema only | `DEFINE IF NOT EXISTS` on all new fields. No `REMOVE FIELD`. No data migration scripts. |
| Google Books API key optional | System falls back gracefully: pipeline skips Google Books step; `cover_image_url` uses Open Library Covers API fallback. |
| `progress_pct` not enforced at SurrealDB layer | `int` type accepts any value; the application layer controls valid transitions (0 → 25 → 50 → 75 → 100). |
| Reading status not enforced at SurrealDB layer | `option<string>` type accepts any string; validation is in the handler and domain layer. ASSUMPTION: this is acceptable for a single-user app where the schema is the binary, not just the DB. |

**Performance budgets (from NFR):**
- Library page with filter/sort: < 200ms for up to 500 books. The `work_created_at` index already exists; a domain-filter query via graph traversal on 500 nodes is well within SurrealDB's embedded performance envelope.
- WebSocket progress updates: < 50ms from worker checkpoint to browser (in-process broadcast via tokio channel, no network hop).

---

## 8. Implementation Sequencing

### Phase 0 — Foundation (blocking everything else, ~0.5 day)

Must be done first, in this order:

1. **Schema migration** — append new fields to `SCHEMA_SQL` in `schema.rs`. Deploy to verify additive migration runs cleanly.
2. **Port trait extensions** — extend `WorkRepo`, `UserRepo`, `external.rs` with new method signatures. Add stub `todo!()` implementations to surreal adapters so the project compiles.
3. **`WsBroadcaster` ordering fix in `main.rs`** — move `WsBroadcaster::new()` to before the NATS worker block. Update `WorkerService::new()` signature to accept `Arc<WsBroadcaster>`.
4. **`AppState` extension** — add `google_books_client: Option<Arc<dyn GoogleBooksPort>>`.

### Phase 1 — Independent tracks (can run in parallel, ~2–3 days)

**Track A: Google Books adapter + worker pipeline (F5, F2)**
- Implement `GoogleBooksClient` in `src/adapters/google_books/mod.rs`
- Implement `WorkRepo::update_progress`
- Extend `WorkerService::run_pipeline` with 4-stage progress + Google Books step
- Extend `WsBroadcaster::broadcast_progress`
- Implement cover fallback logic (Open Library Covers URL construction)
- Update `WorkRecord` / `Work` domain struct with new Google Books fields

**Track B: User profile feature (F1)**
- Implement `UserRepo::find_by_id`, `UserRepo::update_profile`
- Add `src/web/handlers/profile.rs` with all 4 handlers
- Register profile routes in `router.rs`

**Track C: Reading status (S1)**
- Add `validate_reading_status()` in `domain/work.rs`
- Implement `WorkRepo::update_reading_status`
- Extend `works.rs` handler with PATCH handler (shares route with future delete handler)
- Update `Work` domain struct + `WorkRecord` serialization

### Phase 2 — Sequential, depends on Phase 1 (~2–3 days)

These can run in parallel with each other but require Phase 0 to be complete:

**Track D: Delete work (F3)** — depends on schema (Phase 0)
- Implement `WorkRepo::delete_work_cascade` using SurrealDB transaction
- Add DELETE handler in `works.rs`
- Register DELETE route in `router.rs`
- Integration test: verify no orphan edges after delete

**Track E: Library filter + sort (F4, partial)** — depends on Track A completion (cover_image_url)
- Extend `WorkRepo::list_works` to `list_works_filtered(status, domain, sort, order, limit, offset)`
- Implement domain filter via SurrealDB graph traversal
- Add `GET /api/library/domains` handler
- Update library handler to pass filter/sort params from query string

**Track F: Cover images on grid (S2)** — depends on Track A completion
- UI only: render `cover_image_url` in the library grid card component

### Phase 3 — AI Grouping (F4, final feature, ~1 day)

Depends on Phase 2 Track E (needs done works with concepts):
- Extend `GeminiPort` with `group_works(works: Vec<WorkSummary>) -> Result<Vec<GroupResult>, ExternalError>`
- Add `POST /api/library/group` handler with precondition check (>= 5 done books)
- UI: add "Group by AI" button to library view, gated on `done_count >= 5`

### Phase 4 — UI Polish (~1–2 days, can overlap Phase 3)

- Profile page (`/profile`) with display_name, email, password, preferences forms
- Library page: filter dropdowns (status, domain), sort selector
- Book card: reading status badge + cover image
- Book detail page: reading status dropdown, Amazon purchase link, progress bar for in-flight books
- Progress bar UI component (tied to WebSocket progress events)

### Critical Path

```
Phase 0 (foundation)
  → Phase 1 Track A (Google Books + progress)  →  Phase 2 Track E (library filter)  →  Phase 3 (AI grouping)
  → Phase 1 Track B (profile)                  →  Phase 2 Track D (delete)
  → Phase 1 Track C (reading status)           →  Phase 2 Track F (cover images)
```

**Recommended first vertical slice:** Phase 0 + Phase 1 Track A (Google Books + progress broadcast). This is the highest-risk wiring decision (WorkerService + WsBroadcaster) and validates the end-to-end pipeline extension pattern. Every other feature is lower risk once this slice works.

---

## 9. Decisions Log

### ADR-01: WebSocket progress via BroadcastSender injection (Option A)

**Context:** The worker must emit progress updates (0/25/50/75/100%) to WebSocket clients as it completes pipeline stages. Two options were identified in the product brief: inject `Arc<WsBroadcaster>` into WorkerService (Option A), or route through SurrealDB live queries (Option B).

**Decision:** **Option A — inject `Arc<WsBroadcaster>` into `WorkerService` at startup.**

**Rationale:** `WsBroadcaster` already exists in `AppState`. The worker and HTTP server are in the same process and already share state via `Arc`. Option B (live queries) would add SurrealDB subscription overhead, increase latency, and require implementing a live query consumer in the HTTP server layer — all to avoid a simple `Arc<WsBroadcaster>` parameter. The coupling is entirely internal to the binary.

**Consequence (implementation):** `WsBroadcaster::new()` must be called before the NATS worker block in `main.rs`. The current ordering has it after — this is a one-line reorder. `WorkerService::new()` gains an `Arc<WsBroadcaster>` parameter.

---

### ADR-02: Google Books sequential, not parallel (FR-28, US-14)

**Context:** Google Books could be fetched concurrently with Open Library (reducing per-book latency) or sequentially after.

**Decision:** **Sequential: OpenLib → Google Books → Gemini.**

**Rationale:** For a 3–10 books/week personal tool, the latency savings of parallelism (~500ms–2s) are imperceptible. Sequential error attribution is straightforward: if step N fails, we know exactly which API failed. Parallel error handling requires `tokio::join!` with partial success logic and complicates the progress percentage model (25% milestones become 12.5% milestones with parallel fetches). No upside justifies the complexity for this use case.

---

### ADR-03: Reading status as `option<string>` on `work`, validated by application layer

**Context:** S1 adds a `want_to_read / reading / finished` status to books. Schema decision: enum at DB level or string with application-layer validation?

**Decision:** `option<string>` field on `work`, with enum validation in `domain/work.rs`.

**Rationale:** SurrealDB SCHEMAFULL supports `TYPE string` with `ASSERT $value IN ['want_to_read', 'reading', 'finished']`, but ASSUMPTION: this is over-engineering for a single-user tool where the application binary enforces the invariant. Using `option<string>` keeps the schema change minimal and allows `null` (unset) without needing a `None` enum variant at the DB level.

---

### ADR-04: Delete cascade in a single SurrealDB transaction (FR-12–16)

**Context:** DELETE /api/works/{id} must remove the work, its insight(s), and associated `interpreta` and `menciona` edges. Concept nodes and `relacionado_a` edges must be preserved (shared graph knowledge).

**Decision:** Single SurrealDB `BEGIN TRANSACTION` / `COMMIT TRANSACTION` containing all DELETE statements.

**Consequence:** Atomic: either everything is deleted or nothing is. The SurrealDB ACID transaction guarantee (already proven for `GraphWriteRepo`) applies here. The implementation lives entirely in `SurrealWorkRepo::delete_work_cascade`.

---

### ADR-05: Domain filter via SurrealDB graph traversal

**Context:** Library filter by domain requires finding works whose extracted concepts belong to a specific domain.

**Decision:** Use SurrealDB's native graph traversal in the WHERE clause:
```sql
WHERE ->interpreta->insight->menciona->concept.domain CONTAINS $domain
```

**Rationale:** This is the idiomatic SurrealDB approach and leverages the existing graph schema without requiring a join table or denormalization. Performance is acceptable for ≤500 books: the traversal is O(books × avg_concepts), and the `concept.domain` column has cardinality suitable for indexing.

**ASSUMPTION:** The SurrealDB 2.x crate's query execution for this traversal pattern on embedded SurrealKV performs within the 200ms NFR for 500 books. If performance testing reveals otherwise, a domain denormalization approach (store domain directly on `work`) is the fallback — but this trades query performance for schema complexity.

---

### ADR-06: Cover image stored on `work`, not in a separate enrichment table

**Context:** Google Books and Open Library Covers API both provide cover image URLs. These could be stored in a separate `enrichment` table or directly on `work`.

**Decision:** Store on `work` as optional fields (`cover_image_url`, plus page_count, publisher, average_rating, preview_link).

**Rationale:** There is one enrichment source per book (Google Books as primary, OL Covers as fallback). A separate table would require a join for every library page render and complicates the worker pipeline. The `work` table already holds the canonical book record; metadata about the physical book (page count, publisher) belongs there, not in the AI knowledge graph tables.
