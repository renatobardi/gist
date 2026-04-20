# Product Requirements Document: Knowledge Vault

**Version:** 1.1
**Date:** 2026-04-20
**PM Agent:** BMAD Pipeline — Phase 2
**Issue:** REN-99
**Input:** `_bmad/docs/product-brief.md` (Analyst, 2026-04-20)

**Owner decisions incorporated:**
- Concept graph UI is **in v1 scope** with BMW design system (DESIGN.md + preview.html)
- First-run bootstrap: **email + password** (creates admin account on first launch)
- Gemini output: **structured JSON via `response_schema`** — schema fully defined in FR-20
- OQ-05 resolved: **no minimum concept threshold** — accept any number Gemini returns, even 1
- OQ-06 resolved: **free-form strings for domain** — domain filter uses `SELECT DISTINCT domain` at query time

---

## 1. Product Overview

### Vision

A single-binary, self-hosted personal knowledge vault that automatically extracts concepts from books and builds a navigable graph of connected ideas — owned entirely by the user, zero cloud dependencies.

### Problem Summary

Knowledge workers accumulate reading lists but lose the connective tissue between ideas across books. Existing tools are either cloud-only silos or require manual linking. There is no lightweight, self-hosted system that automatically extracts concepts from books and builds a queryable graph of their relationships.

### Target Users

- **Beatriz** — Independent researcher reading 30–50 books/year. Needs cross-domain concept discovery, private data, zero ops overhead.
- **Felipe** — Software architect. Needs API access, self-hosted ARM64 deploy, structured way to query idea relationships across books.

---

## 2. Functional Requirements

### Auth & First-Run Bootstrap

| ID | Requirement |
|----|-------------|
| FR-01 | On first launch, if no users exist, the system must display a first-run setup screen prompting for email and password to create the admin account. |
| FR-02 | Email must be a valid RFC 5321 address. Password must be at minimum 12 characters. |
| FR-03 | Login with email and password must return a JWT (HS256) session token valid for 24 hours. |
| FR-04 | The system must support Personal Access Tokens (PAT). PATs must have the `ens_` prefix and be stored as Argon2id hashes. |
| FR-05 | The system must provide a UI to generate and revoke PATs. |
| FR-06 | All API endpoints except `/health`, `/login`, and the first-run setup endpoint must require authentication (JWT or PAT). |

### Book Ingestion

| ID | Requirement |
|----|-------------|
| FR-07 | The system must accept book submissions by ISBN-10, ISBN-13, or free-text title. |
| FR-08 | On submission, the system must create a `work` record with `status = 'pending'` and return a `work_id` immediately. |
| FR-09 | The system must reject submissions that duplicate an existing `isbn` or `open_library_id`, returning a 409 with the existing `work_id`. |
| FR-10 | The system must publish a `discovery.requested` event to NATS after creating the pending work record. |
| FR-11 | The Open Library API must be credited in the UI on any page that displays book metadata. |

### Async Processing Pipeline

| ID | Requirement |
|----|-------------|
| FR-12 | The worker must consume `discovery.requested` events from NATS JetStream using explicit ack/nack. |
| FR-13 | The worker must fetch book metadata from the Open Library API using the submitted identifier. |
| FR-14 | On transient failures (network, rate limit, timeout), the worker must apply exponential backoff with delays of 5s, 15s, 60s, 180s before final failure. Maximum 5 delivery attempts. |
| FR-15 | After 5 failed attempts, the work record must be marked `status = 'failed'` and a `discovery.failed` event published. |
| FR-16 | On permanent failures (invalid ISBN, unparseable response), the worker must immediately mark the work as `failed` without retrying. |
| FR-17 | The system must expose `POST /api/works/{id}/retry` to re-queue a failed work for processing. |
| FR-18 | Work status must transition through: `pending` → `processing` → `done` or `failed`. |

### Gemini Integration & Structured Output

| ID | Requirement |
|----|-------------|
| FR-19 | The system must call the Gemini API using `response_schema` constrained generation (not free-text parsing). |
| FR-20 | The Gemini `response_schema` must produce the following JSON structure: |

```json
{
  "type": "OBJECT",
  "properties": {
    "summary": { "type": "STRING", "description": "Executive summary of the book in 2-4 sentences." },
    "key_points": {
      "type": "ARRAY",
      "items": { "type": "STRING" },
      "description": "3-7 most important takeaways from the book."
    },
    "concepts": {
      "type": "ARRAY",
      "items": {
        "type": "OBJECT",
        "properties": {
          "name": { "type": "STRING", "description": "Concept name, 1-5 words, title case." },
          "description": { "type": "STRING", "description": "One sentence definition." },
          "domain": { "type": "STRING", "description": "Primary knowledge domain, e.g. Economics, Computer Science, Philosophy." },
          "relevance_weight": { "type": "NUMBER", "description": "Relevance to this book, 0.0-1.0." },
          "related_concepts": {
            "type": "ARRAY",
            "items": {
              "type": "OBJECT",
              "properties": {
                "name": { "type": "STRING" },
                "relation_type": {
                  "type": "STRING",
                  "enum": ["enables", "contrasts_with", "is_part_of", "extends", "related"]
                },
                "strength": { "type": "NUMBER", "description": "Relation strength, 0.0-1.0." }
              },
              "required": ["name", "relation_type", "strength"]
            }
          }
        },
        "required": ["name", "description", "domain", "relevance_weight", "related_concepts"]
      },
      "description": "5-15 key concepts extracted from the book with their relationships."
    }
  },
  "required": ["summary", "key_points", "concepts"]
}
```

| FR-21 | The system prompt sent to Gemini must include: book title, author, Open Library description, and any available subject tags. Prompt target: < 4,000 tokens. |
| FR-22 | The raw Gemini response must be stored in `insight.raw_gemini_response` for auditability. |
| FR-23 | Concepts must be upserted by normalized name (lowercase, trimmed) — no duplicate concept nodes. The system must accept any number of concepts returned by Gemini (including 1); there is no minimum threshold. |
| FR-24 | Concept-to-concept edges (`relacionado_a`) must be created for all `related_concepts` entries returned by Gemini. |

### Knowledge Graph Persistence

| ID | Requirement |
|----|-------------|
| FR-25 | Graph writes must follow the sequence: upsert concepts → create `menciona` edges → create `relacionado_a` edges → create `insight` → create `interpreta` edge → mark work `done`. All in a single SurrealDB transaction where possible. |
| FR-26 | If a concept already exists, only new `relacionado_a` edges may be added; the existing node must not be overwritten. |

### Frontend — Book Library

| ID | Requirement |
|----|-------------|
| FR-27 | The main library view must list all submitted works with title, author, and status badge. |
| FR-28 | The system must push work status updates to the browser via WebSocket. The library view must update reactively without page reload. |
| FR-29 | Each work must have a detail view showing: insight summary, key points list, and the concepts extracted with their relevance weights. |
| FR-30 | The submit form must accept ISBN or title, validate input client-side, and show inline error messages. |

### Frontend — Concept Graph Visualization

| ID | Requirement |
|----|-------------|
| FR-31 | The concept graph page must render an interactive node-edge graph of all concepts and their `relacionado_a` relationships. |
| FR-32 | Each node must be labeled with the concept name and color-coded by domain. |
| FR-33 | Each edge must display the `relation_type` on hover/tap. Edge thickness must reflect `strength`. |
| FR-34 | Clicking a concept node must navigate to a concept detail view listing: description, domain, all books that mention it (via `menciona` ← `interpreta` ← `work`), and related concepts. |
| FR-35 | The graph must support zoom, pan, and node dragging. |
| FR-36 | The graph must support filtering by domain. |

### Frontend — Design System

| ID | Requirement |
|----|-------------|
| FR-37 | All UI components must comply with the BMW design system as specified in `DESIGN.md`: 0px border-radius, BMW Blue (`#1c69d4`) for interactive elements only, near-black (`#262626`) body text, tight line-heights (1.15–1.30), dark hero sections (`#1a1a1a`). |
| FR-38 | The primary font must be Inter (system fallback: Helvetica, Arial) — as applied in `preview.html` since BMWTypeNextLatin is proprietary. |
| FR-39 | All buttons must be sharp-cornered rectangles. Primary buttons use BMW Blue background. Secondary buttons use 1px outline. |

---

## 3. Non-Functional Requirements

| ID | Category | Requirement |
|----|----------|-------------|
| NFR-01 | Deployment | Single binary deployment. No external services except Gemini API at runtime. |
| NFR-02 | Deployment | Binary must cross-compile to `aarch64-unknown-linux-musl` for Oracle Cloud ARM64 Free Tier. |
| NFR-03 | Performance | Binary size must be < 60 MB stripped. |
| NFR-04 | Performance | Cold start (binary invocation to first HTTP response) must be < 2 seconds. |
| NFR-05 | Performance | ISBN submission to graph populated must be < 30 seconds (p95), excluding Gemini API latency variance. |
| NFR-06 | Performance | SurrealDB concept traversal (depth 3, `relacionado_a`) must return results in < 50ms. |
| NFR-07 | Performance | Gemini API call with prompt < 4K tokens must complete within 15 seconds (p95). If exceeded, treat as transient failure and retry. |
| NFR-08 | Security | JWT sessions: HS256, 24-hour expiry, server-side secret stored in config/env. |
| NFR-09 | Security | PAT tokens: `ens_` prefix enforced at validation, stored as Argon2id hash, never logged. |
| NFR-10 | Security | Passwords stored as Argon2id hashes. No plaintext passwords in logs or responses. |
| NFR-11 | Security | All HTTP responses must include security headers: `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Content-Security-Policy`. |
| NFR-12 | Privacy | Zero telemetry, zero analytics callbacks. No data leaves the server except Gemini API requests. |
| NFR-13 | Compliance | Open Library attribution must appear in the UI on any view displaying Open Library data. |
| NFR-14 | Accessibility | All interactive elements must have visible focus states using BMW Focus Blue (`#0653b6`) ring. WCAG 2.1 AA minimum. |
| NFR-15 | Observability | Structured JSON logs (`tracing` crate) with log levels: ERROR, WARN, INFO, DEBUG. Each log line must include `work_id` when processing context is available. |
| NFR-16 | Scalability | System must handle a single user submitting up to 100 books without degradation. Multi-user is out of scope for v1. |

---

## 4. Epics

### E-01 — Auth & Bootstrap
First-run setup, email/password login, JWT sessions, PAT management.
Stories: US-01, US-02, US-03, US-04

### E-02 — Book Ingestion
Submit by ISBN/title, OpenLibrary enrichment, deduplication, work record creation.
Stories: US-05, US-06, US-07

### E-03 — Async Processing Pipeline
NATS JetStream consumer, retry policy, DLQ, status transitions, manual retry.
Stories: US-08, US-09, US-10

### E-04 — Gemini Concept Extraction
Structured `response_schema` call, prompt construction, concept extraction, insight persistence.
Stories: US-11, US-12

### E-05 — Graph Persistence
Concept upsert, edge creation, transactional graph writes to SurrealDB.
Stories: US-13

### E-06 — Frontend: Book Library
Library list view, book detail view, WebSocket status updates, submission form.
Stories: US-14, US-15, US-16

### E-07 — Frontend: Concept Graph
Interactive graph visualization, concept detail, filtering.
Stories: US-17, US-18, US-19

### E-08 — Observability & Error Recovery
Structured logging, health endpoint, manual retry UI, failed work visibility.
Stories: US-20, US-21

---

## 5. User Stories

### US-01 — First-Run Bootstrap
**As Beatriz**, I want the system to guide me through creating an admin account on first launch, so that I can access the vault immediately without needing a separate setup script.

**Acceptance Criteria:**
- **Given** the binary is launched for the first time (no users in DB)
- **When** I navigate to the root URL
- **Then** I am redirected to `/setup` showing an email + password creation form
- **And** submitting valid credentials creates the admin account and logs me in
- **Given** the binary is launched subsequently (a user exists)
- **When** I navigate to `/setup`
- **Then** I am redirected to `/login`

**Dependencies:** FR-01, FR-02, NFR-08
**Effort:** M
**Parallelism:** Sequential — must exist before any other auth story

---

### US-02 — Email/Password Login
**As Felipe**, I want to log in with my email and password and receive a session token, so that I can authenticate subsequent API and UI requests.

**Acceptance Criteria:**
- **Given** I have a registered account
- **When** I submit valid credentials to `POST /auth/login`
- **Then** I receive a JWT in the response body and a `Set-Cookie: session=...` header (HttpOnly, Secure, SameSite=Strict)
- **Given** I submit invalid credentials
- **When** 3 consecutive failures occur for the same email within 5 minutes
- **Then** subsequent attempts return 429 for 5 minutes

**Dependencies:** US-01
**Effort:** M
**Parallelism:** Sequential after US-01

---

### US-03 — PAT Generation
**As Felipe**, I want to generate a Personal Access Token with `ens_` prefix, so that I can authenticate API requests from scripts without exposing my session credentials.

**Acceptance Criteria:**
- **Given** I am logged in
- **When** I request a new PAT via the UI or `POST /api/tokens`
- **Then** the system returns the full token exactly once (never again)
- **And** the token follows the format `ens_<32-char-random>`
- **And** the token is stored as an Argon2id hash in the database

**Dependencies:** US-02, FR-04, FR-05
**Effort:** S
**Parallelism:** Parallel with US-04

---

### US-04 — PAT Revocation
**As Beatriz**, I want to revoke a PAT, so that I can invalidate credentials I no longer use.

**Acceptance Criteria:**
- **Given** I have an active PAT
- **When** I revoke it via the UI or `DELETE /api/tokens/{id}`
- **Then** subsequent API requests using that token return 401

**Dependencies:** US-03
**Effort:** S
**Parallelism:** Parallel with US-03

---

### US-05 — Submit Book by ISBN
**As Beatriz**, I want to submit a book by ISBN, so that the system can look it up and start building my knowledge graph.

**Acceptance Criteria:**
- **Given** I am authenticated
- **When** I submit a valid ISBN-10 or ISBN-13 via `POST /api/works`
- **Then** a work record is created with `status = 'pending'` and I receive the `work_id`
- **Given** I submit an ISBN that already exists
- **When** the server processes the request
- **Then** I receive a 409 with the existing `work_id`

**Dependencies:** US-02, FR-07, FR-08, FR-09, FR-10
**Effort:** M
**Parallelism:** Sequential after US-02; parallel with US-06

---

### US-06 — Submit Book by Title
**As Beatriz**, I want to submit a book by title when I don't have the ISBN, so that the system can search and identify the correct work.

**Acceptance Criteria:**
- **Given** I submit a free-text title
- **When** Open Library returns multiple results
- **Then** the system selects the best match (highest relevance, first result) and creates the work record
- **And** the `open_library_id` is stored on the work record for future deduplication

**Dependencies:** US-02, FR-07
**Effort:** M
**Parallelism:** Parallel with US-05

---

### US-07 — View Work Status
**As Beatriz**, I want to see the real-time processing status of my submitted book, so that I know when my knowledge graph is ready.

**Acceptance Criteria:**
- **Given** I have submitted a book
- **When** I view the library or book detail page
- **Then** a status badge shows the current state: Pending / Processing / Done / Failed
- **And** the badge updates automatically via WebSocket without page reload when the status changes

**Dependencies:** US-05, FR-28, FR-18
**Effort:** M
**Parallelism:** Parallel with US-08 (backend), parallel with US-14 (frontend)

---

### US-08 — NATS Worker: Consume and Process
**As a system operator**, I want the worker daemon to consume `discovery.requested` events and run the full ingestion pipeline, so that books are processed without manual intervention.

**Acceptance Criteria:**
- **Given** a `discovery.requested` event is published
- **When** the worker picks it up
- **Then** it fetches Open Library metadata, calls Gemini, persists the graph, marks the work `done`, and acks the message
- **And** on success, publishes a `discovery.status` event with `work_id` and `done`

**Dependencies:** US-05, FR-12, FR-13
**Effort:** L
**Parallelism:** Sequential after US-05; parallel with US-09

---

### US-09 — NATS Worker: Retry Policy
**As Beatriz**, I want failed processing attempts to retry automatically with backoff, so that transient Gemini or network errors don't permanently fail my books.

**Acceptance Criteria:**
- **Given** a transient failure occurs (network timeout, Gemini rate limit)
- **When** delivery count is < 5
- **Then** the worker nacks with delay: 5s → 15s → 60s → 180s
- **Given** delivery count reaches 5
- **When** the next attempt also fails
- **Then** work is marked `failed`, `discovery.failed` is published, and the message is acked

**Dependencies:** US-08, FR-14, FR-15, FR-16
**Effort:** M
**Parallelism:** Parallel with US-08

---

### US-10 — Manual Retry
**As Beatriz**, I want to manually re-trigger processing of a failed book, so that I can recover from permanent failures after fixing the underlying issue.

**Acceptance Criteria:**
- **Given** a work has `status = 'failed'`
- **When** I click "Retry" in the UI or call `POST /api/works/{id}/retry`
- **Then** the work status resets to `pending` and a new `discovery.requested` event is published

**Dependencies:** US-09, FR-17
**Effort:** S
**Parallelism:** Parallel with US-09

---

### US-11 — Gemini Concept Extraction with Structured Output
**As Felipe**, I want the system to extract concepts from my books using a structured JSON schema, so that concept data is consistent and machine-readable without brittle text parsing.

**Acceptance Criteria:**
- **Given** a book context (title, author, OL description, subject tags)
- **When** the worker calls the Gemini API
- **Then** `response_schema` is used (not free-text generation)
- **And** the response conforms to the schema in FR-20: `summary`, `key_points`, `concepts[]` with `name`, `description`, `domain`, `relevance_weight`, `related_concepts[]`
- **And** the raw Gemini response JSON is stored in `insight.raw_gemini_response`
- **Given** Gemini returns a response that violates the schema
- **When** the worker processes it
- **Then** it treats the failure as a permanent failure (no retry)

**Dependencies:** US-08, FR-19, FR-20, FR-21, FR-22
**Effort:** L
**Parallelism:** Parallel with US-09

---

### US-12 — Concept & Edge Persistence
**As Beatriz**, I want concepts to be deduplicated across books so that "Distributed Systems" from one book connects to the same node as "Distributed Systems" from another.

**Acceptance Criteria:**
- **Given** Gemini returns a concept named "Distributed Systems"
- **When** the worker persists the graph
- **Then** if a `concept` node with that normalized name already exists, the existing node is reused (not duplicated)
- **And** `menciona` and `relacionado_a` edges are created referencing the existing node
- **And** the existing concept's `description` and `domain` are NOT overwritten by subsequent books

**Dependencies:** US-11, FR-23, FR-24, FR-25, FR-26
**Effort:** M
**Parallelism:** Sequential after US-11

---

### US-13 — Transactional Graph Write
**As a system operator**, I want the full graph write (insight + concept upserts + all edges) to succeed or fail atomically, so that partial graph state never persists.

**Acceptance Criteria:**
- **Given** the worker is persisting a book's extracted graph
- **When** any step in the write sequence fails (e.g., edge creation errors)
- **Then** the entire transaction is rolled back and work status remains `processing`
- **And** the failure is treated as a transient error (subject to retry policy)

**Dependencies:** US-12, FR-25
**Effort:** M
**Parallelism:** Sequential after US-12

---

### US-14 — Book Library View
**As Beatriz**, I want to see all my submitted books in a clean list with their processing status, so that I have a single overview of my knowledge vault.

**Acceptance Criteria:**
- **Given** I am logged in
- **When** I navigate to `/`
- **Then** I see a list of all works with title, author, status badge
- **And** the page uses the BMW design system: dark header (`#1a1a1a`), white content area, Inter font, 0px border-radius
- **And** an "Add Book" button (BMW Blue) opens the submission form inline or navigates to `/add`

**Dependencies:** US-07, FR-27, FR-37, FR-38
**Effort:** M
**Parallelism:** Parallel with US-15, US-16

---

### US-15 — Book Detail View
**As Beatriz**, I want to view the extracted insight and concepts for a book, so that I can quickly recall the key ideas without re-reading.

**Acceptance Criteria:**
- **Given** a work has `status = 'done'`
- **When** I navigate to `/works/{id}`
- **Then** I see: book metadata (title, author, ISBN), insight summary, key points list, and a concept list with relevance weight indicators
- **And** Open Library attribution appears on this page

**Dependencies:** US-12, FR-29, NFR-13
**Effort:** M
**Parallelism:** Parallel with US-14, US-16

---

### US-16 — Book Submission Form
**As Beatriz**, I want a simple form to submit books by ISBN or title, so that I can add books to my vault without touching the API directly.

**Acceptance Criteria:**
- **Given** I am on the submission form
- **When** I enter an invalid ISBN (wrong check digit or wrong length)
- **Then** an inline error appears before submission ("Invalid ISBN-13 — check digit mismatch")
- **Given** I submit a valid ISBN
- **When** the request succeeds
- **Then** the form clears and the new book appears in the library list with "Pending" status

**Dependencies:** US-05, US-06, FR-30
**Effort:** M
**Parallelism:** Parallel with US-14, US-15

---

### US-17 — Interactive Concept Graph Page
**As Felipe**, I want to explore my entire concept graph visually, so that I can discover unexpected connections across the books I've read.

**Acceptance Criteria:**
- **Given** concepts exist in the database
- **When** I navigate to `/graph`
- **Then** I see all concept nodes and `relacionado_a` edges rendered as an interactive graph
- **And** nodes are color-coded by `domain`
- **And** edge thickness reflects `strength`
- **And** I can zoom, pan, and drag nodes

**Dependencies:** US-12, FR-31, FR-32, FR-33, FR-35
**Effort:** XL
**Parallelism:** Parallel with US-18, US-19

---

### US-18 — Concept Detail View
**As Beatriz**, I want to click on a concept node and see all books that mention it plus its related concepts, so that I can trace ideas across my reading history.

**Acceptance Criteria:**
- **Given** I click on a concept node
- **When** the detail panel or page opens
- **Then** I see: concept name, description, domain, a list of books that mention it (via `menciona` ← `interpreta` ← `work` traversal), and its related concepts with relation types

**Dependencies:** US-17, FR-34
**Effort:** M
**Parallelism:** Parallel with US-17

---

### US-19 — Graph Domain Filter
**As Felipe**, I want to filter the concept graph by domain, so that I can focus on concepts from a specific knowledge area without the visual noise of unrelated domains.

**Acceptance Criteria:**
- **Given** I am on the `/graph` page
- **When** I select one or more domains from the filter
- **Then** only nodes matching those domains are visible (and their edges)
- **And** the graph re-renders without a page reload
- **And** the domain list is populated dynamically via `SELECT DISTINCT domain FROM concept` (free-form strings, no fixed enum)

**Dependencies:** US-17, FR-36
**Effort:** S
**Parallelism:** Parallel with US-17

---

### US-20 — Health Endpoint
**As Felipe**, I want a `/health` endpoint that reports binary version and SurrealDB connectivity, so that I can monitor the service from my Oracle Cloud infrastructure.

**Acceptance Criteria:**
- **Given** the service is running and DB is connected
- **When** I call `GET /health`
- **Then** I receive `{"status": "ok", "version": "...", "db": "connected"}` with HTTP 200
- **Given** SurrealDB is not reachable
- **When** I call `GET /health`
- **Then** I receive HTTP 503 with `{"status": "degraded", "db": "disconnected"}`

**Dependencies:** None
**Effort:** S
**Parallelism:** Fully parallel

---

### US-21 — Failed Works Dashboard
**As Beatriz**, I want to see which books failed processing and why, so that I can decide whether to retry or remove them.

**Acceptance Criteria:**
- **Given** one or more works have `status = 'failed'`
- **When** I view the library
- **Then** failed works show a "Failed" badge and an error message (truncated to 80 chars)
- **And** a "Retry" button is available for each failed work
- **And** clicking Retry calls `POST /api/works/{id}/retry`

**Dependencies:** US-10, US-14, FR-17
**Effort:** S
**Parallelism:** Parallel with US-14

---

## 6. MVP Scope

### In Scope for v1

- First-run email/password bootstrap and admin account creation
- JWT session + PAT authentication
- Book submission by ISBN-13, ISBN-10, or title
- Open Library metadata enrichment
- Async NATS JetStream pipeline with exponential backoff retry
- Gemini `response_schema` concept extraction
- SurrealDB graph persistence: `work`, `insight`, `concept` nodes + 3 edge types
- Leptos SSR + WASM frontend with BMW design system
- Book library view with real-time WebSocket status updates
- Book detail view (insight, key points, concepts)
- **Interactive concept graph visualization** (canvas-based, zoom/pan/filter)
- Concept detail view with cross-book relationship traversal
- Single binary deployment on `aarch64-unknown-linux-musl`
- `/health` endpoint
- Structured JSON logging

### Out of Scope for v1

- Multi-user support — single admin account only; v2 decision
- Non-book content types (articles, PDFs, podcasts) — v2 expansion
- Multiple LLM providers — Gemini only; interface is defined but not wired for others
- Mobile-native app
- Public sharing or graph export (JSON/CSV/GraphML)
- Graph editing (manual concept linking, renaming) — read-only graph in v1
- Full-text search across insights — v2 after search index design
- Highlights or annotations per book — v2
- Email notifications on ingestion completion — polling/WebSocket is sufficient for v1

### Minimum Viable Feature Set

The core hypothesis: **"A single-binary self-hosted tool can automatically build a useful concept graph from books."**

The MVP is valid if: a user can submit 5 books by ISBN, have them all reach `done` status, and see meaningful concept-to-concept connections in the graph UI within 30 minutes of first launch on a fresh Oracle ARM64 VM.

---

## 7. Open Questions

| # | Question | Owner | Blocking |
|---|----------|-------|---------|
| OQ-01 | What graph rendering library is compatible with Leptos WASM? Options: Cytoscape.js (JS interop via `wasm-bindgen`), petgraph + custom SVG rendering (pure Rust), or a Leptos-native canvas approach. This is an Architect decision. | Architect | US-17 |
| OQ-02 | Should SurrealDB graph writes use `BEGIN TRANSACTION` / `COMMIT` in SurrealQL 3.0, or rely on individual statement atomicity? SurrealKV embedded transaction semantics need validation. | Architect | US-13 |
| OQ-03 | NATS embedded server: is `async-nats` with the `server` feature sufficient, or must the NATS server binary be embedded as a subprocess? If subprocess, the single-binary constraint is partially broken. This is the highest-risk assumption in the brief. | Architect | E-03 |
| OQ-04 | Login rate limiting: implement in Axum middleware (in-memory, resets on restart) or persist lockout state in SurrealDB? In-memory is simpler but loses state on restart. | Architect | US-02 |
| OQ-05 | ~~Minimum concept threshold~~ **Resolved:** No minimum threshold. Accept whatever Gemini returns (including 1 concept). A "poor" record is preferable to a failure or retry loop burning Gemini tokens. See FR-23. | ~~Owner~~ ✅ | — |
| OQ-06 | ~~Enum vs free-form domain values~~ **Resolved:** Free-form strings. Domain filter (US-19) uses `SELECT DISTINCT domain FROM concept` at query time. Future normalization can be done via SurrealDB aggregation or Gemini post-processing. See FR-36, US-19. | ~~Owner~~ ✅ | — |

---

*End of PRD v1.1 — Knowledge Vault*
