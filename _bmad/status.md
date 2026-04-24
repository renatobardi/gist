# BMAD Pipeline Status — Knowledge Vault

| Phase | Agent | Status | Date |
|-------|-------|--------|------|
| Analysis | Analyst | ✅ Complete | 2026-04-20 |
| PM | PM | ✅ Complete | 2026-04-20 |
| Architecture | Architect | ✅ Complete | 2026-04-20 |
| UX Design | UX Designer | ✅ Complete | 2026-04-20 |
| Development | Dev | ⏳ Not started | — |

---

# BMAD Pipeline Status — MVP 2

| Phase | Agent | Status | Date |
|-------|-------|--------|------|
| Analysis | Analyst | ✅ Complete | 2026-04-24 |
| PM | PM | 🔄 Awaiting owner input | — |
| Architecture | Architect | ⏳ Not started | — |
| Development | Dev | ⏳ Not started | — |

## MVP 2 — Analysis Phase Deliverables
- `_bmad/docs/product-brief-mvp2.md` — Product Brief (6 core features + recommendations, constraints, risks, open questions)

## MVP 2 — Open Questions (blocking PM phase)
1. Goodreads import: does owner have an existing library to migrate?
2. AI grouping: ephemeral only, or persist as named collections?
3. Google Books API: does owner have a GCP project?
4. Reading status field: include in MVP 2?

---

## MVP 1 — Analysis Phase Deliverables
- `_bmad/docs/product-brief.md` — Product Brief + Initial System Design (SurrealQL schema, Rust Traits, NATS strategy)

## PM Phase Deliverables
- `_bmad/docs/prd.md` — Full PRD: 24 FRs, 16 NFRs, 8 epics, 21 user stories

## Architecture Phase Deliverables
- `_bmad/docs/architecture.md` — Full architecture document: tech stack, system design, data model, API contracts, project structure, implementation sequencing

## Owner Decisions Incorporated
- Concept graph UI: **in v1 scope**, BMW design system applied
- First-run bootstrap: **email + password** admin account creation
- Gemini output: **structured JSON** via `response_schema` (schema defined in FR-20)
- NATS strategy: **Self-Extracting Binary** (aarch64-musl NATS server embedded in Rust binary, spawned via std::process)
- Graph rendering: **Cytoscape.js** via wasm-bindgen; orthogonal edges (`taxi`), rectangular nodes; BMW style
- Gemini model: **Gemini 3 Flash Preview** as primary; code-level field defaulting for optional fields
