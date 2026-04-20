# User Flows

This document maps the critical user journeys for Knowledge Vault.

## Flow 1: First-Run Setup (US-01)
**Persona:** Felipe (Software Architect)
**Goal:** Initialize the application and create the admin account.

| Step | User Action | System Response | Next State |
|------|-------------|-----------------|------------|
| 1 | Navigate to `/` | Detects no users; redirects to `/setup`. | Setup Page |
| 2 | Enter email and password (min 12 chars) | Validates input; creates user in DB. | Redirect to `/login` |
| 3 | Enter credentials on `/login` | Validates; returns JWT; sets session cookie. | Redirect to `/` (Library) |

## Flow 2: Adding a Book & Real-time Processing (US-16, US-17, US-18)
**Persona:** Beatriz (Researcher)
**Goal:** Add a new book and watch it being processed into the graph.

| Step | User Action | System Response | Next State |
|------|-------------|-----------------|------------|
| 1 | Click "Add Book" on `/` | Opens submission form. | Modal/Inline Form |
| 2 | Enter ISBN and submit | API returns 202; NATS event published. | Library View |
| 3 | Wait for processing | WS pushes status updates: `pending` → `processing` → `done`. | Library View (Reactive) |
| 4 | Click on book card | Navigate to detail view. | `/works/{id}` |
| 5 | View summary & concepts | Displays Gemini-extracted insights. | Detail View |

## Flow 3: Exploring the Knowledge Graph (US-19, US-20, US-21)
**Persona:** Beatriz (Researcher)
**Goal:** Discover connections between concepts across different books.

| Step | User Action | System Response | Next State |
|------|-------------|-----------------|------------|
| 1 | Navigate to `/graph` | Renders Cytoscape graph with nodes and edges. | Graph Page |
| 2 | Hover over an edge | Displays relation type (e.g., "enables") and strength. | Graph Page |
| 3 | Click on a concept node | Opens detail panel; shows related books and concepts. | `/graph/concepts/{id}` |
| 4 | Use Domain Filter | Dynamically filters graph to show only selected domains. | Graph Page (Filtered) |

## Flow 4: Error Handling & Retry (US-17)
**Persona:** Felipe (Software Architect)
**Goal:** Handle a failed ingestion and re-trigger processing.

| Step | User Action | System Response | Next State |
|------|-------------|-----------------|------------|
| 1 | View book with `failed` status | Shows error message badge. | Library View |
| 2 | Click "Retry" button on card | API calls `POST /api/works/{id}/retry`. | Status set to `pending` |
| 3 | Monitor status | Pipeline restarts processing. | Library View |
