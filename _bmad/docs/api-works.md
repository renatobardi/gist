# Works API

This section details the API endpoints for managing "works" — primarily the ingestion of book references into the Knowledge Vault system.

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
