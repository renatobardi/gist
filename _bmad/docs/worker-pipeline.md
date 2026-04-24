# Worker Pipeline: Progress Tracking & Google Books Integration

**Date:** 2026-04-24  
**Status:** Implemented (REN-147)  
**Feature:** S01-02 — Worker Pipeline: Progress Tracking & Google Books Integration (Phase 1 Track A)

---

## Overview

The Knowledge Vault worker pipeline processes book submissions asynchronously through a 4-stage pipeline with real-time progress tracking. Each stage emits progress checkpoints (0%, 25%, 50%, 75%, 100%) that are persisted to the work record and broadcast via WebSocket, enabling live UI updates. The pipeline integrates Google Books for metadata enrichment and falls back gracefully when the Google Books API key is unavailable.

## Architecture

### Pipeline Stages

```
Stage 0 (0%)   → Fetching metadata from Open Library
       ↓
Stage 1 (25%)  → Enriching with Google Books
       ↓
Stage 2 (50%)  → Extracting concepts with Gemini
       ↓
Stage 3 (75%)  → Writing to knowledge graph
       ↓
Stage 4 (100%) → Complete
```

### Data Flow Diagram

```
┌─────────────────────────────────────────────────────────────┐
│ POST /api/works (ISBN)                                      │
│ → Create work{status: pending}                              │
│ → Publish discovery.requested to NATS                       │
│ → Return 202 {work_id, status: pending}                     │
└────────────────┬────────────────────────────────────────────┘
                 │
                 ▼
    ┌────────────────────────────────────────┐
    │      NATS Worker (background)          │
    └────────────────┬───────────────────────┘
                     │
        ┌────────────┴───────────────┐
        │                            │
        ▼ (0%)                      ▼
   [OpenLibrary]           [Update progress_pct=0]
   Fetch metadata          [Broadcast via WS]
        │                            │
        ├────────────────────────────┘
        │
        ▼ (25%)
    [Optional: Google Books]
    Enrich metadata
    (cover, pages, rating)
        │
        ├─── Update progress_pct=25
        │    Broadcast via WS
        │
        ▼ (50%)
    [Gemini API]
    Extract concepts
        │
        ├─── Update progress_pct=50
        │    Broadcast via WS
        │
        ▼ (75%)
    [SurrealDB Transaction]
    • UPSERT concepts
    • CREATE edges (menciona, relacionado_a)
    • CREATE insight
    • UPDATE work status = "done"
        │
        ├─── Update progress_pct=75
        │    Broadcast via WS
        │
        ▼ (100%)
    [Complete]
    Update progress_pct=100
    Broadcast status="done" via WS
```

### Progress Persistence & WebSocket

Each stage emits progress updates via two mechanisms:

1. **Database Persistence**: `work.progress_pct` and `work.last_action` are written to SurrealDB
2. **Real-time Broadcast**: WebSocket message sent to all connected clients with type `"work_progress"`

This dual approach ensures:
- Progress survives server restart (persisted to DB)
- UI receives live updates without polling (WebSocket broadcast)
- Clients that connect after the fact can query `GET /api/works/{id}` to fetch the latest progress

### Google Books Integration

**Conditional Execution:**
- Only runs for **ISBN-based submissions** (not title-based)
- Skipped if `KV_GOOGLE_BOOKS_API_KEY` environment variable is absent
- Non-fatal: errors in Google Books do not fail the entire pipeline

**Behavior by Error Type:**

| Error Type | Behavior | Log Level | Pipeline Impact |
|------------|----------|-----------|-----------------|
| `SkippedOptional` | API key not set, step skipped | DEBUG | None — continue |
| `Transient` (timeout, connection error) | Warn and continue | WARN | None — continue |
| `Permanent` (invalid API key, bad request) | Warn and continue | WARN | None — continue |
| Success | Persist metadata to work record | INFO | Metadata available in work details |
| No results | Log and continue | DEBUG | None — continue |

**Metadata Persisted on Success:**

When Google Books returns results, the following fields are written to the work record:
- `cover_image_url` — URL to book cover image
- `page_count` — Total number of pages
- `publisher` — Publishing company
- `average_rating` — Community rating (0–5 scale)
- `preview_link` — Link to preview content

If Google Books returns no cover image, Open Library's cover service provides a fallback URL (automatic, no additional call needed).

**Example:**
```rust
// In worker.rs, after Google Books fetch
match gb_client.fetch_by_isbn(&isbn).await {
    Ok(Some(meta)) => {
        // Persist cover_image_url (falls back to Open Library if empty)
        work_repo.update_google_books_metadata(
            &work_id,
            meta.cover_image_url.as_deref(),  // Fallback applied by adapter
            meta.page_count.map(|p| p as i32),
            meta.publisher.as_deref(),
            meta.average_rating,
            meta.preview_link.as_deref(),
        ).await
    }
    Ok(None) => { /* no results, continue */ }
    Err(ExternalError::SkippedOptional(_)) => { /* no API key, skip */ }
    Err(ExternalError::Transient(_)) => { /* timeout, warn and continue */ }
    Err(ExternalError::Permanent(_)) => { /* invalid API, warn and continue */ }
}
```

## API & WebSocket Events

### Progress Updates via WebSocket

When connected to `GET /ws`, clients receive JSON messages for each progress update:

```json
{
  "type": "work_progress",
  "work_id": "550e8400-e29b-41d4-a716-446655440000",
  "progress_pct": 50,
  "last_action": "Extracting concepts with Gemini"
}
```

**Event Types:**
- `"type": "work_progress"` — Progress checkpoint (emitted at 0%, 25%, 50%, 75%, 100%)
- `"type": "work_status"` — Status change (emitted as "processing", "done", "failed")

### Querying Progress via REST

After work submission, retrieve the latest progress by calling:

```
GET /api/works/{work_id}
Authorization: Bearer <JWT or PAT>
```

Response includes:
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "The Pragmatic Programmer",
  "author": "David Thomas, Andrew Hunt",
  "isbn": "978-0201616224",
  "status": "processing",
  "progress_pct": 50,
  "last_action": "Extracting concepts with Gemini",
  "created_at": "2026-04-24T12:00:00Z",
  "updated_at": "2026-04-24T12:00:30Z",
  ...
}
```

## Error Handling

### Transient vs. Permanent Errors

**Transient Errors** (retried with exponential backoff):
- Connection timeouts
- Service unavailable (5xx)
- Rate limiting (429)

**Permanent Errors** (fail immediately, no retry):
- Invalid ISBN or ISBN not found
- Invalid Gemini API response (schema violation on required fields)
- Database constraint violations

### Worker Error Classification

```rust
pub enum WorkerError {
    Transient(String),   // Retry with backoff
    Permanent(String),   // Fail immediately
}
```

### Work Status Lifecycle

```
pending    → processing  → done      (success)
         ╲             ╱
          → failed     (permanent error)
```

On permanent error:
1. Work status set to `"failed"`
2. `error_msg` field populated with error description
3. WebSocket broadcast: `{"type": "work_status", "work_id": "...", "status": "failed"}`

On transient error:
1. Work status remains `"processing"`
2. NATS consumer retries with backoff (1s → 2s → 4s → 8s, max 30s, 5 attempts)
3. If all retries exhausted, work marked as `"failed"` (see Note below)

**Note:** There is a known limitation where transient errors exhausted at max retries do not trigger a final status update to `"failed"`. This is tracked as a TODO in `worker.rs` and will be fixed in a future iteration.

### Retry Policy

| Attempt | Delay | Cumulative Time |
|---------|-------|-----------------|
| 1 | 1s | 1s |
| 2 | 2s | 3s |
| 3 | 4s | 7s |
| 4 | 8s | 15s |
| 5 | 16s (capped at 30s) | 45s |

## Development Guide

### Testing the Pipeline

**Unit tests:** `cargo test --lib app::worker`
- Progress emission captures all 5 checkpoints (0, 25, 50, 75, 100)
- Google Books non-fatal errors are handled correctly (SkippedOptional, Transient, Permanent all result in success)
- Error classification (Transient vs. Permanent)

**Integration tests:** `cargo test --test '*' -- --include-ignored`
- Full pipeline with mocked Gemini (uses `mockito` HTTP mock)
- Google Books adapter with different error scenarios
- WebSocket broadcast messages
- Database persistence of progress and metadata

### Manual Testing Checklist

1. **Basic flow (ISBN):**
   - `POST /api/works` with ISBN
   - Monitor progress via WebSocket or polling `GET /api/works/{id}`
   - Verify all 5 progress checkpoints appear (0, 25, 50, 75, 100)
   - Confirm final status is "done" and insight + concepts are persisted

2. **Google Books enrichment:**
   - Submit ISBN with `KV_GOOGLE_BOOKS_API_KEY` set
   - Verify `cover_image_url`, `page_count`, `publisher`, `average_rating`, `preview_link` are populated
   - Repeat without API key (or set to empty) — verify work still completes (skips Google Books)

3. **Error handling:**
   - Invalid ISBN → immediate 422 error
   - Duplicate ISBN → immediate 409 error
   - Timeout in OpenLibrary → transient error, retried
   - Timeout in Gemini → transient error, retried (after 5 attempts, marked failed)

4. **Title-based submission:**
   - `POST /api/works` with `identifier_type: "title"`
   - Verify Google Books step is skipped (even if API key present)
   - Confirm rest of pipeline completes normally

## Configuration

### Environment Variables

| Variable | Type | Default | Required | Notes |
|----------|------|---------|----------|-------|
| `KV_GEMINI_API_KEY` | string | — | Yes | Required for worker to process books; absence causes worker loop not to spawn |
| `KV_GOOGLE_BOOKS_API_KEY` | string | — | No | Optional; if absent, Google Books step is skipped gracefully |
| `KV_GEMINI_MODEL` | string | `gemini-2.5-flash` | No | Model version for concept extraction |

### Database Schema

New fields added to `work` table (REN-157 Phase 0):

```surrealql
DEFINE FIELD IF NOT EXISTS progress_pct ON work TYPE option<float>;
DEFINE FIELD last_action ON work TYPE option<string>;
```

Existing fields also used:
- `status` — work processing status (pending|processing|done|failed)
- `error_msg` — populated on failure
- `updated_at` — updated on each progress checkpoint

## Known Limitations

1. **Exhausted transient errors:** After 5 retries, work status remains "processing" instead of transitioning to "failed". Workaround: Implement a consumer callback to signal permanent failure to the handler. See TODO in `worker.rs` line ~120.

2. **Sequential OpenLibrary + Gemini calls:** Calls are not parallelized intentionally to simplify error attribution during debugging. Parallelization can be added in v2 if latency becomes problematic.

3. **Title-based submissions skip Google Books:** By design, only ISBN-based submissions enrich via Google Books. Title-to-ISBN lookup is not implemented in v1.

## Future Enhancements

1. **Batch processing:** Process multiple works in parallel (currently one worker task).
2. **Graceful degradation:** Cache concept graphs for failed Gemini calls, allow manual override.
3. **Progress webhooks:** POST progress updates to a client-provided URL instead of (or in addition to) WebSocket.
4. **Resumable uploads:** If a work fails mid-pipeline, allow resume-from-checkpoint without starting over.
