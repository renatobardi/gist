# Product Brief: Knowledge Vault MVP 2

**Version:** 1.0
**Date:** 2026-04-24
**Analyst Agent:** BMAD Pipeline — Analysis Phase
**Issue:** REN-139 (MVP 2)
**PM Phase Issue:** REN-140

---

## 1. Project Vision

MVP 2 deepens Knowledge Vault rather than pivoting it. The core pipeline (ingest → enrich → graph) is proven and stable. MVP 2 makes every interaction feel complete: users can manage their identity, see exactly what is happening to in-flight books, delete mistakes, find books quickly, and discover where to buy them. It also expands metadata quality by adding Google Books as a second enrichment source.

**Problem Statement:** MVP 1 delivered the core pipeline but left visible gaps in the user experience — no way to delete wrong submissions, no granular progress visibility during processing, no ability to sort or filter the library, and a single metadata source. These gaps create friction for the primary user in daily use.

---

## 2. User Personas

Single-user personal tool. Two behavioral modes:

**Knowledge Curator** (primary daily mode)
- Submits 3–10 books/week
- Wants zero ambiguity about processing state
- Frustrated when a wrongly-submitted book can't be deleted
- Needs to know if a book is stuck vs. still processing

**Auditor** (periodic library review mode)
- Reviews the full library monthly
- Needs sort/filter/delete without friction
- Wants to discover purchase links for books not yet owned
- Values clean visual presentation with cover images

---

## 3. Key Goals

### Must-have goals
- Users can manage their account profile and preferences
- Users can see granular processing progress with retry capability
- Users can delete library items
- Users can sort and filter the library

### Explicit non-goals (MVP 2)
- Multi-user support (remains single-user)
- Bulk operations (retry/delete multiple books) — deferred to MVP 3
- NYT Bestseller Discovery tab — deferred to MVP 3
- ISBNdb integration (paid API, not justified for personal use)
- Goodreads CSV import — deferred pending owner confirmation of need

---

## 4. Feature Scope

### F1 — User Profile (`/profile`)
**What:** New `/profile` page for the logged-in user.
**Includes:**
- Display name (editable)
- Email change (requires current password confirmation)
- Password change (requires current password confirmation)
- UI preferences: theme (light/dark), default library sort order

**Data model impact:** Extend existing `user` record with `display_name: string` and `preferences: object` fields. Additive, no migration needed.

### F2 — Processing Status with Retry
**What:** Granular progress visibility during book processing, plus retry capability.
**Includes:**
- Progress percentage: 0% → 25% → 50% → 75% → 100% at 4 checkpoints:
  1. Open Library fetch complete (25%)
  2. Google Books fetch complete (50%)
  3. Gemini concept extraction complete (75%)
  4. Graph write complete (100%)
- Last-action log line: human-readable status string for the current step
- Retry button: surfaces existing `POST /api/works/{id}/retry`; shown only when status is `error` or book is stuck in `pending`/`processing` for > 5 minutes

**Data model impact:** Add `progress_pct: integer` (0–100) and `last_action: string` to `Work` records.

**Architecture decision point (for Architect):** The worker must emit progress updates to connected WebSocket clients. The worker and HTTP server currently share the same binary but not a reference to the WebSocket broadcaster. Two options:
- Option A: Pass `BroadcastSender` into `WorkerService` state at startup
- Option B: Route progress updates through SurrealDB live queries (worker writes, server reads)

This is a non-trivial wiring decision — the Architect must make an explicit call.

### F3 — Delete Library Items
**What:** Ability to remove a book from the library.
**Includes:**
- `DELETE /api/works/{id}` endpoint
- Cascade: Work → Insight → `menciona` edges deleted
- Concept nodes **preserved** (shared across books — deleting them would corrupt the graph)
- UI: confirmation dialog before deletion

### F4 — Sort / Filter / AI Grouping
**What:** Navigation and organization tools for the library.
**Includes:**
- **Sort** (client-side): date added, title, author, processing status
- **Filter** (server-side): by status (`pending`/`processing`/`done`/`error`); by domain (distinct values from `SELECT DISTINCT domain`)
- **AI Grouping**: new `POST /api/library/group` endpoint; calls Gemini with the list of done books + their concepts; returns ephemeral cluster labels (not persisted); button visible only when ≥ 5 books have `status = done`

**AI grouping persistence decision:** Ephemeral (query result, no storage). Saved collections would be a new entity with CRUD + schema and roughly double the implementation cost — deferred to MVP 3.

### F5 — Multi-source Metadata (Google Books)
**What:** Add Google Books API as a secondary enrichment source.
**Includes:**
- New fields populated from Google Books: cover image URL, page count, publisher, average rating, preview link
- `KV_GOOGLE_BOOKS_API_KEY` env var — optional; system falls back to Open Library-only if not set
- Open Library Covers API used as cover image fallback when Google Books is unavailable or returns no cover
- ISBNdb: deferred (paid API, not needed for personal use)

**Data model impact:** New optional fields on `Work`/`Insight`. Additive, no migration needed.

### F6 — Amazon Purchase Link
**What:** Direct link to buy the book on Amazon.
**Includes:**
- Static URL pattern: `https://www.amazon.com/s?k={ISBN13}`
- No API key required, no affiliate setup, no maintenance
- Displayed on book detail view when ISBN is available

---

## 5. Additional Features (Recommended)

These were not explicitly requested but are low-cost and high-value:

| ID | Feature | Recommendation |
|----|---------|----------------|
| S1 | Reading status field: `want_to_read` / `reading` / `finished` | **Include in MVP 2** — low implementation cost, high daily-use value |
| S2 | Cover images on library grid | **Include in MVP 2** — free once Google Books (F5) is integrated |
| S4 | Library export (CSV/JSON) | **Consider for MVP 2** — low cost, useful for backup |
| S3 | Bulk operations (retry/delete multiple) | Defer to MVP 3 |
| S5 | NYT Bestseller Discovery tab | Defer to MVP 3 |

---

## 6. Competitive Landscape

| Tool | Relevance |
|------|-----------|
| **Readwise** | Cloud-only, subscription-based; strong highlights but no graph or self-hosting |
| **Obsidian + plugins** | Local-first but requires manual linking; no auto-extraction pipeline |
| **Notion** | Cloud database, manual entry; no processing automation |
| **Goodreads** | Rich social features and library management but no concept extraction or graph |
| **Open Library** | Primary data source (not a competitor); free, open, comprehensive |
| **Google Books API** | Secondary data source; adds cover images and metadata not in Open Library |

**Differentiation**: Knowledge Vault's combination of self-hosted single-binary deploy + automated concept graph extraction is not available in any comparable tool. MVP 2 closes the remaining UX gap versus cloud-hosted alternatives.

---

## 7. Constraints

| Type | Constraint |
|------|-----------|
| **Technical** | Single aarch64-musl binary target must be preserved — no new runtime dependencies |
| **Technical** | SurrealDB schema changes must be additive — no destructive migrations |
| **Technical** | Google Books API key is optional — system must degrade gracefully to Open Library only |
| **Infrastructure** | Amazon purchase links require no new API keys or infrastructure |
| **Business** | This is a single-user personal tool — multi-user and RBAC are out of scope |
| **Timeline** | Due date was 2026-04-23 (passed) — feature prioritization into must-have vs. nice-to-have is critical |

---

## 8. Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|-----------|
| WebSocket progress wiring is more complex than estimated | Medium | High | Architect must explicitly decide broadcaster wiring vs. live query pattern before Dev starts |
| Google Books API rate limits impact processing pipeline | Low | Medium | Treat as optional enrichment; continue pipeline on API failure |
| Delete cascade leaves orphaned `menciona` edges | Medium | Medium | Implement and test cascade delete in a single transaction |
| AI grouping Gemini call times out on large libraries | Low | Low | Add timeout + error state; button is already gated at ≥ 5 books |
| Feature scope exceeds timeline | High | High | Owner must explicitly cut features into must-have / nice-to-have before Architect starts |

---

## 9. Assumptions Requiring Validation

| # | Assumption | Risk if wrong |
|---|-----------|--------------|
| A1 | Owner does not have a Goodreads library to migrate | If wrong, CSV import becomes must-have for MVP 2 |
| A2 | Ephemeral AI grouping is sufficient (no persistence) | If wrong, saved collections roughly double F4 implementation cost |
| A3 | Owner has or will create a Google Cloud project for Books API | If wrong, F5 reduces to Open Library Covers API only (lower cover coverage) |
| A4 | Reading status field (S1) is desired | Owner should confirm before Architect designs the data model |

---

## 10. Success Metrics

| Metric | Target |
|--------|--------|
| User can delete a wrongly submitted book | < 3 clicks from library view |
| Processing progress visible without page refresh | Real-time via WebSocket |
| Retry a stuck/failed book | < 2 clicks from book detail |
| Library sort + filter response time | < 200ms for libraries up to 500 books |
| Cover images loaded | ≥ 80% of done books have a cover image |
| Amazon link accuracy | 100% (deterministic ISBN-based URL) |

---

## 11. Technology Considerations

The Architect should consider the following trade-offs. No stack decisions are made here.

**Progress broadcast to WebSocket clients:**
- Option A — Pass `BroadcastSender` into `WorkerService` at startup (tight coupling, simpler)
- Option B — Route through SurrealDB live queries (looser coupling, adds live query overhead)

**Google Books integration:**
- Could be added to the existing `WorkerService` pipeline (sequential after Open Library call)
- Or parallelized with Open Library fetch (reduces latency, increases complexity)

**AI Grouping endpoint:**
- Could reuse the existing Gemini adapter with a new prompt
- Response is ephemeral (no SurrealDB write needed)

**Reading status:**
- Simple enum field on `Work` — no new table or relations required

---

## 12. Recommended Next Steps for PM

1. **Get owner answers** to the 4 open questions (Goodreads import, AI grouping persistence, Google Books API key, reading status) — these are scope blockers, not optional
2. **Owner must prioritize** features into: MVP 2 must-have / MVP 2 nice-to-have / MVP 3 — the due date is past; a clear cut is required before the Architect starts
3. **Flag the WebSocket progress wiring** as an architecture decision point in the PRD — do not let the Dev discover it mid-implementation
4. **Recommend including S1 (reading status) and S2 (cover images)** in MVP 2 — both are free riders on existing MVP 2 work
