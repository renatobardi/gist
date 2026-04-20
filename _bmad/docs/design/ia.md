# Information Architecture

## 1. Site Map

- `/setup` (Public/First-run) — Admin creation
- `/login` (Public) — Authentication
- `/` (Private) — Book Library (Dashboard)
    - `/works/{id}` — Book Detail & Insights
- `/graph` (Private) — Interactive Concept Graph
    - `/graph/concepts/{id}` — Concept Detail Panel
- `/api-keys` (Private) — Personal Access Token management (FR-05)

## 2. Navigation Structure

### Primary Navigation (Header)
- **Library** (Link to `/`)
- **Graph** (Link to `/graph`)
- **API Keys** (Link to `/api-keys`)
- **User Logout** (Action)

### Secondary/Contextual Navigation
- **Add Book** (Action on Library page)
- **Retry Ingestion** (Action on failed Book card)
- **Domain Filter** (Action/Toggle on Graph page)

## 3. URL Structure

- `/` — Root, shows list of all works.
- `/works/:id` — Details for a specific work.
- `/graph` — Full-screen graph view.
- `/graph/concepts/:id` — Detail view for a concept (can be an overlay or side-panel).

## 4. Content Hierarchy

### Page: Library
1. Page Title ("Knowledge Vault")
2. Global Navigation
3. "Add Book" Primary Action
4. List of Books (Title, Author, Status Badge)

### Page: Book Detail
1. Global Navigation
2. Back to Library
3. Book Title & Author
4. Insight Summary (2-4 sentences)
5. Key Points (Bulleted list)
6. Concepts List (Name, Domain, Relevance)

### Page: Concept Graph
1. Global Navigation
2. Domain Filter (Multi-select)
3. Graph Canvas (Full screen minus header/filter)
4. Node/Edge Tooltips on Hover
5. Selected Concept Detail (Side panel)
