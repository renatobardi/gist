# Product Brief — Knowledge Vault MVP 2

**Initiative:** REN-139 — MVP 2  
**Phase:** Analysis  
**Date:** 2026-04-23  
**Analyst:** Analyst Agent  

---

## 1. Project Vision and Problem Statement

Knowledge Vault MVP 1 delivered a working personal knowledge management system: ingest books by ISBN or title, extract AI concepts, visualize a knowledge graph. It works. What it lacks is the polish and completeness that would make someone want to use it daily.

MVP 2 is not a pivot — it is depth. The core pipeline (ingest → enrich → graph) is solid. The goal now is to make every interaction feel finished: a user knows who they are in the system, can see exactly what is happening to their books and why, can manage their library with confidence (including deleting mistakes), can find books quickly, and can discover where to buy a book they just added.

The secondary goal is information quality. OpenLibrary is good but not comprehensive — pages, cover images, publisher details, and page count are often missing. A multi-source metadata strategy closes that gap without adding operational complexity.

---

## 2. Target Audience and User Personas

This is a single-user personal tool. There is one persona by design.

**Persona 1 — The Knowledge Curator (primary)**  
A self-directed learner who reads broadly across disciplines: economics, computer science, philosophy, history. They ingest books faster than they can read them, use the concept graph to discover non-obvious connections between ideas, and want their tool to feel like a second brain, not a toy project. They are technical enough to self-host but expect the UI to not punish them for it. They are frustrated by tools that lose track of state, show spinning spinners with no explanation, or force them to page-refresh to find out if their book finished processing.

**Persona 2 — The Auditor (secondary)**  
Same person, different mode. They periodically review their library to clean up duplicates, remove books added by mistake, or reorganize by theme. They need bulk-friendly interactions: sort, filter, group, and delete without friction.

---

## 3. Key Goals

**G1 — Complete user identity**: The user can see and edit their own profile (display name, email change, password change, UI preferences). First-run setup created a minimal admin account; MVP 2 promotes that to a real profile.

**G2 — Transparent processing**: Every book in a non-terminal state (pending, processing) shows granular progress. Not just a status badge — a percentage, a last-action log line, and a clear retry path for anything stuck in error or hung in pending.

**G3 — Library management parity**: A library is only useful if it stays accurate. The user must be able to delete any book (with confirmation), with full cascade of related insights and concept-mention edges.

**G4 — Discoverability within the library**: Sort by date added, title, author, or processing status. Filter by status and/or domain. AI-assisted grouping clusters the library by thematic similarity using the concept data already in the graph.

**G5 — Richer metadata**: Supplement OpenLibrary with Google Books API (cover images, page count, publisher, categories, preview link) and optional ISBNdb (extended identifiers). Graceful fallback — if a source is unavailable, the others carry the load.

**G6 — Buy path**: Every book detail page shows a direct Amazon search link for that ISBN/title. No affiliate complexity for a personal tool — a direct `amazon.com/s?k=<ISBN>` link is sufficient and zero-maintenance.

**Non-goals:**
- No multi-user support (single admin account remains the model)
- No Goodreads API integration (Goodreads shut down public API in 2020)
- No mobile app or PWA
- No paid affiliate link infrastructure
- No social/sharing features

---

## 4. Competitive Landscape

Knowledge Vault is not in competition with consumer products, but knowing what exists sets a bar for quality.

| Tool | Strengths | Weaknesses |
|------|-----------|------------|
| **Readwise Reader** | Highlights, annotations, newsletters, mobile | Cloud-only, subscription, no concept graph |
| **Obsidian + Dataview** | Local, extensible, powerful queries | Manual data entry, no AI enrichment, no ISBN pipeline |
| **Calibre** | Mature, deep metadata, self-hosted | No AI, no concept extraction, desktop-only UI, 2001-era UX |
| **Bookwyrm** | Self-hosted social book tracker | No AI, requires federation infrastructure, no concept graph |
| **Goodreads** | Massive catalog, social graph | No AI, no self-hosting, API gone, privacy concerns |

**Knowledge Vault's moat**: The concept graph. No other self-hosted tool extracts cross-book conceptual relationships automatically. MVP 2 protects that moat by making the surrounding experience good enough that users do not churn back to Calibre or Notion.

---

## 5. Constraints

**Technical:**
- Single-binary target (aarch64-musl) must be preserved. No new runtime dependencies.
- Google Books API requires an API key (free tier: 1,000 requests/day). Must be optional — OpenLibrary remains primary; Google Books enriches.
- ISBNdb is paid ($10+/month) — treat as optional premium enrichment, not required.
- SurrealDB schema changes must be additive (no breaking migrations against existing data).
- The NATS processing pipeline is the insertion point for progress tracking — percentage must come from the worker, not the HTTP layer.

**Business / Operational:**
- Self-hosted, personal use only. No GDPR/CCPA obligations beyond common sense.
- Oracle Cloud Free Tier ARM64 deployment must continue working unchanged.
- No new cloud services that incur cost (Google Books API free tier is acceptable).

**Timeline:**
- The owner set this as urgent with a due date of 2026-04-23 (today). This brief should unblock the PM phase immediately.

---

## 6. Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Google Books API coverage gaps (some ISBNs return no results) | High | Low | Fall back to OpenLibrary; missing fields are acceptable |
| Processing percentage tracking requires NATS state changes | Medium | Medium | Model progress as discrete steps (1/4, 2/4, 3/4, 4/4); no need for real-time sub-step tracking |
| Delete cascade corrupts concept graph if concept is referenced by other books | Medium | High | Only delete concept-mention edges, not concept nodes; add an orphan cleanup job |
| AI grouping produces poor clusters for small libraries (< 5 books) | Medium | Low | Show grouping button only when library has ≥ 5 books; include fallback "No clear groups found" message |
| Amazon link rot (ASIN changes, regional availability) | Low | Low | Use ISBN-based search URL (`amazon.com/s?k=ISBN`) — always finds the book even if ASIN shifts |
| User profile expansion increases schema complexity | Low | Low | Add `user_preferences` as a JSON/record field; avoids schema migration |

---

## 7. Assumptions That Need Validation

- **A1**: Google Books API returns cover image URLs for ≥ 80% of books the user will add. If not, the cover image feature delivers less value than expected.
- **A2**: The existing SurrealDB `user` table can be extended with a `display_name` and `preferences` field without a destructive migration. Needs schema verification.
- **A3**: The NATS consumer can publish intermediate state updates to the WebSocket broadcaster without creating a tight coupling. Needs architecture review.
- **A4**: The owner does not need a Goodreads import path. If they have an existing library in Goodreads, this becomes a must-have, not a non-goal. Flagged for PM clarification.
- **A5**: AI grouping via a single Gemini call (batch of all books + their top concepts) stays within token limits for libraries up to ~200 books. Needs token estimation.

---

## 8. Feature Set — Detailed Breakdown

### F1 — User Profile Page (`/profile`)

**What**: A dedicated page (and API endpoint) where the logged-in user can view and edit their account: display name, email, password. Also exposes UI preferences (theme: light/dark/system; default sort order for library; preferred AI model if configurable).

**Why now**: First-run creates a bare email+password account. There is no way to change either. This is a basic completeness gap.

**Scope**:
- `GET /profile` — display current user info
- `POST /profile` — update display name, email (re-verify uniqueness), password (current password required)
- Preferences stored as structured JSON in SurrealDB user record
- No avatar upload (out of scope — no file storage)

### F2 — Processing Status Enrichment

**What**: While a book is in `pending` or `processing` state, show a progress percentage and a human-readable last-action line. Add a one-click retry for any book in `error` or stuck in `pending` beyond a threshold (e.g., > 10 minutes).

**Why now**: The current spinner with no explanation is the most common source of confusion in beta feedback for tools like this. Users cannot tell if their book is queued, stuck, or silently failed.

**Scope**:
- Extend `Work` record with `progress_pct: u8` (0–100) and `last_action: Option<String>`
- Worker publishes progress updates at discrete checkpoints:
  - 0%: queued
  - 25%: OpenLibrary fetch complete
  - 50%: Google Books enrichment complete (or skipped)
  - 75%: Gemini extraction complete
  - 100%: graph write complete
- WebSocket broadcast on each progress update (already wired)
- Retry button visible on any book in `error` or `pending` (existing `POST /api/works/{id}/retry` endpoint, surfaced in UI)
- Log line examples: "Fetching from Open Library…", "Enriching with Google Books…", "Extracting concepts with Gemini…", "Writing to knowledge graph…"

### F3 — Delete Library Item

**What**: A delete button on each book card and the book detail page. Confirmation dialog. Deletes the Work, its Insight, and all concept-mention edges (`menciona`) originating from that Insight. Does NOT delete Concept nodes (they may be shared by other books).

**Why now**: There is no way to remove a book added by mistake. This is a basic library management requirement.

**Scope**:
- `DELETE /api/works/{id}` — new endpoint
- Cascades: delete Work → delete related Insight(s) → delete `menciona` edges from those Insights
- Does not cascade to Concept nodes (orphan concepts are acceptable; add cleanup endpoint if needed later)
- UI: confirmation modal with book title ("Delete 'Thinking Fast and Slow'?")
- Soft delete option (mark as deleted, hide from UI) considered and rejected — adds complexity without benefit for a personal tool

### F4 — Sort, Filter, and AI Grouping

**Sort** (client-side, no new API endpoints needed if library is loaded in full):
- By: date added (default desc), title (A-Z), author (A-Z), status (grouped)

**Filter** (server-side or client-side):
- By status: all, pending, processing, done, error
- By domain: any domain tag present in associated concepts (derived from graph)

**AI Grouping** (new feature):
- Button: "Group by Theme" on the library page
- On click: `POST /api/library/group` — sends list of Work IDs + their top concepts to Gemini
- Gemini returns a JSON array of `{ group_name: String, work_ids: Vec<String> }`
- UI renders books clustered under group headings
- Groups are ephemeral (not persisted) — re-run produces new groups
- Show button only when library has ≥ 5 books with `status = 'done'`

**Scope clarification**: Grouping leverages the concept/domain data already in SurrealDB. No new AI pipeline — just a new Gemini call in the HTTP handler.

### F5 — Multi-Source Metadata Enrichment

**Primary (existing)**: OpenLibrary — title, author, description, subjects. No change.

**Secondary (new)**: Google Books API
- **What it adds**: Cover image URL, page count, publisher, published date, categories, average rating, preview link
- **Integration**: Optional enrichment step added to the processing pipeline (step 2 of 4, between OpenLibrary and Gemini)
- **API key**: Required; configured via new env var `KV_GOOGLE_BOOKS_API_KEY`. If not set, step is skipped silently.
- **Rate limit**: 1,000 requests/day (free tier). Sufficient for personal use.
- **Coverage**: ~70–80% of modern books have Google Books entries. Fall through gracefully when no match.

**Tertiary (future/optional)**: ISBNdb — too expensive for personal use without a clear gap. Deferred.

**New data stored on Work**:
- `cover_url: Option<String>` — from Google Books or Open Library Covers API
- `page_count: Option<u32>`
- `publisher: Option<String>`
- `published_date: Option<String>`
- `google_books_id: Option<String>`
- `average_rating: Option<f32>`
- `preview_link: Option<String>`

**Open Library Covers API**: Already available without a key: `https://covers.openlibrary.org/b/isbn/{ISBN}-L.jpg`. Use as fallback if Google Books has no cover.

### F6 — Amazon Purchase Link

**What**: On every book detail page, a "Buy on Amazon" button that opens Amazon's search for that ISBN.

**Implementation**: Static URL construction — no API, no affiliate tag:
- `https://www.amazon.com/s?k={ISBN13}` — searches Amazon for the ISBN
- Falls back to `https://www.amazon.com/s?k={title}+{author}` if no ISBN

**Why not affiliate links**: Affiliate links require joining the Amazon Associates program, adding tracking tags, and complying with disclosure requirements. For a personal self-hosted tool, the overhead is not worth it. The owner can add their affiliate tag later if they want.

**No API key, no cost, no maintenance.** A pure frontend link.

---

## 9. Additional Suggestions

Beyond the explicitly requested features, the following are worth considering for MVP 2 or a near-term MVP 3:

**S1 — Reading Status** (`want_to_read` / `reading` / `finished`): A simple enum field on Work. Adds a layer of personal context that makes the library useful as a reading tracker, not just a knowledge archive. Low implementation cost.

**S2 — Cover Images on Library Grid**: If F5 (Google Books) is implemented, cover images are available. Show them on book cards. This single visual change makes the library feel dramatically more polished. Directly depends on F5.

**S3 — Bulk Operations**: Select-all, select-by-filter, then "Retry selected" or "Delete selected". Useful when many books failed processing and need batch retry. Moderate implementation cost; can be deferred to MVP 3.

**S4 — Export Library**: `GET /api/works/export?format=csv|json` — export the full library with metadata and key points. Useful for backup and interoperability. Low cost.

**S5 — NYT Bestseller Discovery**: The NYT Books API (free, requires key) provides weekly bestseller lists with ISBNs. Could power a "Discover" tab — "Here are this week's bestsellers; add to library with one click." Medium cost; high delight. Suggested for MVP 3.

**S6 — Concept Similarity Search**: Given a concept you care about, find the most related concepts across your library. The graph data is already there — this is a query, not a new pipeline. Could be surfaced as a search input on the concept graph page.

---

## 10. Initial Technology Considerations

The following surface trade-offs without prescribing decisions — those belong to the Architect.

**Progress tracking state**: Options include (a) embedding progress in the SurrealDB Work record and broadcasting via the existing WebSocket, (b) maintaining progress in NATS metadata, or (c) an in-memory progress map in the HTTP process. Option (a) is the most consistent with the existing architecture and survives process restarts. The Architect should verify whether SurrealDB's live query or WebSocket broadcast is a better fit.

**Google Books enrichment placement**: This can be a new adapter in the existing processing pipeline or a post-hoc enrichment job. Pipeline placement means the data is available immediately after processing; post-hoc means enrichment can be added to already-processed books. The Architect should evaluate whether retroactive enrichment of existing Work records is a priority.

**AI grouping prompt design**: A single prompt with all books and their top 3 concepts probably stays within Gemini context limits for libraries up to ~200 books (rough estimate: 200 books × 200 tokens = 40K tokens, well within Gemini 2.5 Flash's 1M context). For larger libraries, batching or summarization may be needed. The Architect should define the grouping prompt template and output schema.

**Delete cascade in SurrealDB**: SurrealDB supports graph traversal in queries. The cascade delete (Work → Insights → menciona edges) can likely be expressed in a single SurrealQL transaction. The Architect should verify the optimal query pattern.

**Amazon link construction**: This is purely a frontend concern. No backend work required unless the owner later requests server-side affiliate tag injection or link shortening.

---

## 11. Success Metrics

| Metric | Definition | Target |
|--------|------------|--------|
| Profile completion rate | % of sessions where user has set a display name | 100% (single user — they will set it once) |
| Processing clarity | User can describe what is happening to any in-flight book without refreshing | Subjective; validated by owner in review |
| Delete reliability | Delete operation leaves zero orphan Insight/menciona records | 100% verified by integration test |
| Cover image coverage | % of done books with a cover image | ≥ 70% |
| Grouping usefulness | Owner uses AI grouping feature and finds groups meaningful | Validated in owner review after delivery |
| Amazon link success | Link for any book with ISBN leads to correct Amazon search results | 100% (deterministic URL construction) |

---

## 12. Recommended Next Steps for PM Agent

1. **Clarify reading status requirement (A4)**: Does the owner have an existing Goodreads library they want to import? If yes, a CSV import feature becomes a must-have and affects the Data Model section of the PRD.
2. **Clarify AI grouping persistence**: Should groups be saved as named collections, or is ephemeral grouping sufficient? This changes the data model significantly.
3. **Confirm Google Books API key availability**: If the owner does not have or does not want to create a Google Cloud project, F5 reduces to Open Library Covers API only (cover image + nothing else). Still worthwhile but less impactful.
4. **Prioritize F1–F6 into epics**: Suggested priority: F3 (delete) → F2 (progress) → F6 (Amazon link) → F5 (Google Books) → F4 (sort/filter) → F1 (profile) → AI grouping (last, as it depends on F4 UI scaffold).
5. **Capture suggestions S1 and S2** in the backlog — they are low-cost and high-value enough that they should ship with MVP 2 if dev capacity allows.
6. **Write PRD** with 24+ functional requirements covering all F1–F6 features, including new API endpoints, schema changes, error handling, and the three NATS progress checkpoints.
