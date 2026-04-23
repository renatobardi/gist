# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Knowledge Vault is a single-binary Personal Knowledge Management system built in Rust. It ingests book references (via ISBN or title), enriches them through the Open Library API, extracts concepts via Gemini API, and stores everything as an interconnected knowledge graph in SurrealDB.

The application lives under `knowledge-vault/`. All commands below assume `cd knowledge-vault` first.

## Tech Stack

- **Backend**: Rust + Axum (HTTP server)
- **Database**: SurrealDB with embedded SurrealKV (zero external dependencies)
- **Messaging**: NATS JetStream (async processing pipeline)
- **LLM**: Google Gemini API (concept extraction)
- **Auth**: JWT (HS256, 24h expiry, httponly cookie) + Personal Access Tokens (`ens_` prefix, Argon2id hash at rest)
- **Deploy target**: Oracle Cloud Free Tier, ARM64

## Commands

```bash
# Development
cargo watch -x run

# Build
cargo build --release       # Single binary ~50-80MB on ARM64

# Tests
cargo test --lib                                      # Unit tests only
cargo test --test '*' -- --include-ignored            # All integration tests (needs SurrealDB + NATS)
cargo test --lib domain::work                         # Single module
cargo test test_name -- --nocapture                   # Single test with output
```

Integration tests require env vars: `KV_JWT_SECRET`, `KV_GEMINI_API_KEY` (optional), and a running SurrealDB/NATS instance.

## Environment Variables

| Variable | Default | Notes |
|---|---|---|
| `KV_JWT_SECRET` | — | Required, 32+ chars |
| `KV_GEMINI_API_KEY` | — | Required to enable the worker |
| `KV_DATA_DIR` | `./data` | SurrealKV storage path |
| `KV_PORT` | `8080` | HTTP listen port |
| `KV_NATS_URL` | `nats://127.0.0.1:4222` | |
| `KV_GEMINI_MODEL` | `gemini-2.5-flash` | |

The worker (NATS consumer) only spawns if `KV_GEMINI_API_KEY` is set.

## Architecture

Hexagonal architecture — domain is free of framework dependencies; adapters implement port traits.

```
src/
├── domain/       # Pure entities: Work, User, Insight, Concept (no I/O)
├── ports/        # Trait definitions: repository.rs, external.rs, messaging.rs
├── adapters/     # Implementations: surreal/, nats/, gemini/, openlib/
├── app/          # Application services: worker.rs (NATS consumer)
└── web/          # HTTP layer: router.rs, state.rs, handlers/, middleware/
```

### Async Processing Flow

```
POST /api/works → NatsPublisher (discovery.requested) → NatsConsumer
→ WorkerService: OpenLib metadata fetch → Gemini concept extraction
→ SurrealDB: create insight + upsert concepts + graph relations
→ WebSocket broadcast → work status = "done"
```

### Domain Model

Graph nodes: **Work** (ISBN-unique), **Insight** (AI-generated summary + key points), **Concept** (extracted term with category)

Graph edges:
- `interpreta`: Work → Insight
- `menciona`: Insight → Concept (with `relevance_weight`)
- `relacionado_a`: Concept ↔ Concept (with `relation_type` + `strength`)

### Error Classification in Worker

`WorkerError::Transient` (timeout, rate limit, connection refused) → NATS `Nak` with exponential backoff (1s→2s→4s→8s, capped 30s, max 5 attempts)

`WorkerError::Permanent` (schema violation, invalid JSON) → NATS `Term` immediately

### Non-Obvious Patterns

- **SurrealDB Thing IDs**: `thing.to_string()` wraps IDs in backticks; use the local `thing_id_to_string()` helper in each repo to strip them.
- **Auth token sources**: `AuthenticatedUser` extractor accepts Bearer header OR session cookie — both paths go through the same JWT validation.
- **Optional NATS**: If the publisher is `None` in `AppState`, work submission returns 503. This is intentional — the system is not degraded-mode friendly; it fails loudly.
- **Rate limiting**: Login is capped at 3 failures per email per 5-minute window, tracked in the `login_attempts` SurrealDB table (no in-memory state).
- **Work status lifecycle**: `pending` → `processing` → `done` (or `error` with `error_msg` populated).
- **OpenLib + Gemini calls are sequential**, not parallel — intentional to simplify error attribution during processing.

## Key Design Decisions

- **SurrealDB embedded (SurrealKV)**: Eliminates external DB dependency for single-binary deployment
- **NATS for decoupling**: HTTP request returns immediately after publishing; processing failures don't block the API
- **Hexagonal pattern**: Swap Gemini for another LLM by implementing `GeminiPort` — domain layer unchanged
- **Argon2id everywhere**: Both user passwords and PAT hashes use Argon2id (OWASP 2026 params: m=65536, t=3, p=1)
