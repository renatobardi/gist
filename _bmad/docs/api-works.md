# Works API

This section details the API endpoints for managing "works" — primarily the ingestion of book references into the Knowledge Vault system.

## Overview

When a work (book) is submitted for processing:

1. The system creates a work record with status `"pending"` and returns immediately (202 Accepted)
2. An asynchronous worker processes the submission through a 4-stage pipeline with real-time progress tracking:
   - **Stage 0 (0%):** Fetch metadata from Open Library
   - **Stage 1 (25%):** Enrich with Google Books (optional, non-fatal)
   - **Stage 2 (50%):** Extract concepts with Gemini API
   - **Stage 3 (75%):** Write results to knowledge graph
   - **Stage 4 (100%):** Complete
3. Progress updates are persisted to the database (`progress_pct`, `last_action`) and broadcast via WebSocket in real-time
4. Final status is `"done"` (success) or `"failed"` (error); the insight and concepts are available via `GET /api/works/{id}`

See [Worker Pipeline documentation](./worker-pipeline.md) for detailed information on progress tracking, Google Books integration, and error handling.

## Ingest a Work by ISBN

This endpoint allows for the submission of new book references using their ISBN (International Standard Book Number). The system will process the ISBN, fetch book metadata, and initiate the knowledge graph generation process. Duplicate ISBNs will be rejected.

```
POST /api/works
Authorization: Bearer <JWT or PAT>
Content-Type: application/json

{
  "identifier": "978-0321765723",
  "identifier_type": "isbn"
}
```

### Request Parameters

| Parameter         | Type   | Required | Constraints                                  | Description                                                                  |
|-------------------|--------|----------|----------------------------------------------|------------------------------------------------------------------------------|
| `identifier`      | string | Yes      | Valid ISBN-10 or ISBN-13 (with/without hyphens) | The ISBN of the book to be ingested.                                         |
| `identifier_type` | string | Yes      | Must be `"isbn"`                             | Specifies the type of identifier being provided. Currently only `"isbn"` is supported. |

### Response: 202 Accepted

```json
{
  "work_id": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
  "status": "pending"
}
```

Returned when the ISBN is valid, not a duplicate, and the work ingestion process has been successfully initiated. The `status` field indicates that the work is awaiting asynchronous processing.

| Field     | Type         | Description                                                              |
|-----------|--------------|--------------------------------------------------------------------------|
| `work_id` | string (UUID)| Unique identifier for the newly created work.                            |
| `status`  | string       | The current processing status of the work. Will be `"pending"` initially. |

### Response: 409 Conflict

```json
{
  "error": "duplicate_work",
  "message": "A work with the provided ISBN already exists.",
  "work_id": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
  "status": "done"
}
```

Returned when a work with the provided ISBN already exists in the system. The `work_id` of the existing duplicate work is provided.

### Response: 422 Unprocessable Content

```json
{
  "error": "invalid_isbn",
  "message": "The provided ISBN is invalid. Please ensure it is a valid ISBN-10 or ISBN-13."
}
```

Returned when the provided `identifier` is not a valid ISBN-10 or ISBN-13 (e.g., incorrect format, invalid check digit).

```json
{
  "error": "invalid_input",
  "message": "Invalid identifier_type. Only 'isbn' is currently supported."
}
```

Returned when `identifier_type` is not `"isbn"`.

### Response: 401 Unauthorized

Returned when:
- No Authorization header provided
- Authorization header format is invalid
- JWT is expired or invalid
- PAT is revoked or invalid

---

## Monitoring Work Progress

### Real-time Progress via WebSocket

Once a work is submitted, you can receive real-time progress updates by connecting to the WebSocket endpoint:

```
GET /ws
Authorization: Bearer <JWT or PAT>
Upgrade: websocket
```

The server broadcasts progress messages as JSON:

```json
{
  "type": "work_progress",
  "work_id": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
  "progress_pct": 50,
  "last_action": "Extracting concepts with Gemini"
}
```

**Event Types:**
- `"type": "work_progress"` — Progress checkpoint (emitted at 0%, 25%, 50%, 75%, 100%)
- `"type": "work_status"` — Status change (emitted when status becomes "processing", "done", or "failed")

**Example Status Event:**
```json
{
  "type": "work_status",
  "work_id": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
  "status": "done"
}
```

### Polling Progress via REST

Query the work details to get the current progress state:

```
GET /api/works/{work_id}
Authorization: Bearer <JWT or PAT>
```

Response includes:
```json
{
  "id": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
  "title": "The Pragmatic Programmer",
  "author": "David Thomas, Andrew Hunt",
  "isbn": "978-0201616224",
  "status": "processing",
  "progress_pct": 50,
  "last_action": "Extracting concepts with Gemini",
  "created_at": "2026-04-24T12:00:00Z",
  "updated_at": "2026-04-24T12:00:30Z",
  "cover_image_url": "https://covers.openlibrary.org/b/id/7778352-M.jpg",
  "page_count": 352,
  "publisher": "Addison-Wesley Professional",
  "average_rating": 4.5,
  "preview_link": "https://books.google.com/books?id=..."
}
```

**Progress Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `progress_pct` | number (0-100) | Current pipeline progress as percentage: 0% = starting, 25% = Open Library fetch complete, 50% = Google Books complete, 75% = Gemini extraction complete, 100% = pipeline complete |
| `last_action` | string | Description of the current stage (e.g., "Fetching metadata from Open Library") |

**Metadata Fields** (populated by Google Books during Stage 1):

| Field | Type | Description |
|-------|------|-------------|
| `cover_image_url` | string \| null | URL to the book cover image (from Google Books, with Open Library fallback) |
| `page_count` | integer \| null | Total number of pages in the book |
| `publisher` | string \| null | Publishing company name |
| `average_rating` | number \| null | Community rating (0–5 scale) |
| `preview_link` | string \| null | URL to preview content (typically Google Books preview) |

**Note:** If a work fails during processing, `status` will be `"failed"` and `error_msg` will contain the error description.

---

## Error Responses

All error responses follow the format:

```json
{
  "error": "<error_code>",
  "message": "<human_readable_message>",
  "status": <http_status_code>
}
```

Common error codes:

| Code              | Status | Meaning                                      |
|-------------------|--------|----------------------------------------------|
| `duplicate_work`  | 409    | A work with this ISBN already exists.        |
| `invalid_isbn`    | 422    | The provided ISBN is not valid.              |
| `invalid_input`   | 422    | Invalid identifier_type or other input error.|
| `unauthorized`    | 401    | Missing or invalid authentication.           |
| `internal_error`  | 500    | Server error.                                |
