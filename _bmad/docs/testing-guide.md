# Knowledge Vault Testing Guide

**Version:** 1.0  
**Last Updated:** 2026-04-21  
**Audience:** Developers, QA Engineers, End Users

This guide covers how to test Knowledge Vault across all test levels: unit tests, integration tests, and manual end-to-end (E2E) scenarios.

## Table of Contents

1. [Test Architecture](#test-architecture)
2. [Unit Tests](#unit-tests)
3. [Integration Tests](#integration-tests)
4. [Manual Testing](#manual-testing)
5. [End-to-End Testing](#end-to-end-testing)
6. [Performance Testing](#performance-testing)
7. [Security Testing](#security-testing)
8. [Test Data Setup](#test-data-setup)

---

## Test Architecture

### Testing Strategy

Knowledge Vault follows a **pyramid testing approach**:

```
        ╔═════════╗
        ║   E2E   ║  Manual + Browser testing (few, slow)
        ╠═════════╣
        ║   Integration ║  DB + NATS testing (moderate)
        ╠═════════╣
        ║  Unit   ║  Logic & domain testing (many, fast)
        ╚═════════╝
```

### Test Scope

| Level | Scope | Tools | Speed | Coverage |
|-------|-------|-------|-------|----------|
| **Unit** | Domain logic, validation, utilities | `cargo test --lib` | < 1s | 80% target |
| **Integration** | SurrealDB, NATS, external APIs | `cargo test --test '*'` | 10-30s | Adapters |
| **E2E** | Full user journeys via browser | Manual + Playwright | Minutes | Critical paths |

### Excluded from Mocking

- **SurrealDB**: All adapter tests run against real SurrealKV instances (in-process)
- **NATS**: Integration tests use a real embedded NATS server
- **No in-memory test doubles for data layers** — this prevents mock/prod divergence

---

## Unit Tests

Unit tests verify domain logic, error handling, and edge cases without external dependencies.

### Running Unit Tests

```bash
# All unit tests
cargo test --lib

# Specific test module
cargo test --lib domain::concept

# Single test function
cargo test --lib domain::concept::test_normalize_name

# With output (useful for debugging)
cargo test --lib -- --nocapture
```

### Test Coverage

```bash
# Generate coverage report (requires `cargo-tarpaulin`)
cargo install cargo-tarpaulin
cargo tarpaulin --lib --out Html

# View report
open tarpaulin-report.html
```

### Test Categories

#### 1. Domain Model Tests

**File**: `knowledge-vault/src/domain/` (each module has `#[cfg(test)]` section)

**Examples**: ISBN validation, concept name normalization, work status transitions

```bash
cargo test --lib domain::work::
cargo test --lib domain::concept::
cargo test --lib domain::user::
```

**Key test scenarios**:

| Scenario | Test | Expected |
|----------|------|----------|
| Valid ISBN-10 | `test_valid_isbn10_check_digit` | Passes validation |
| Invalid ISBN-10 | `test_invalid_isbn10_check_digit` | Returns `InvalidIsbn` error |
| Concept name normalization | `test_normalize_name_trimmed_lowercase` | `"  MyName  "` → `"myname"` |
| Empty concept name | `test_normalize_empty_name` | Returns validation error |
| Work status enum transitions | `test_work_status_pending_to_processing` | Valid state change |
| Invalid work status transition | `test_work_status_done_to_pending` | Invalid, rejected |

#### 2. Error Classification Tests

**File**: `knowledge-vault/src/app/worker.rs`

Tests that errors are correctly classified as **Transient** (retry) vs. **Permanent** (fail immediately).

```bash
cargo test --lib app::worker::error_classification
```

**Examples**:

| Error Type | Classification | Retry Behavior |
|------------|-----------------|----------------|
| Gemini API timeout | Transient | Exponential backoff |
| Invalid JSON response | Permanent | No retry, work fails |
| Database connection error | Transient | Retry with backoff |
| Missing required field in Gemini response | Permanent | No retry, work fails |

#### 3. Authentication Tests

**File**: `knowledge-vault/src/domain/user.rs`

```bash
cargo test --lib domain::user::
```

**Examples**:

- JWT token encoding/decoding
- PAT token format validation
- Password strength validation
- Argon2id hashing

---

## Integration Tests

Integration tests verify that adapters (database, messaging, external APIs) work correctly with the domain layer.

### Running Integration Tests

```bash
# All integration tests
cargo test --test '*'

# Specific integration test
cargo test --test integration_surreal_work_repo

# With output for debugging
cargo test --test '*' -- --nocapture

# With ignored tests (some tests are expensive, marked #[ignore])
cargo test --test '*' -- --include-ignored
```

### Test Structure

Integration tests are in `knowledge-vault/tests/` with submodules in `tests/integration/`.

```
tests/
├── integration.rs              # Test harness
├── integration/
│   ├── surreal_work_repo.rs    # SurrealDB work repository tests
│   ├── surreal_concept_repo.rs # SurrealDB concept repository tests
│   ├── nats_consumer.rs        # NATS JetStream consumer tests
│   └── gemini_mock.rs          # Gemini API mock tests
└── fixtures/
    └── gemini_response.json    # Mock response data
```

### Database Integration Tests

**File**: `tests/integration/surreal_work_repo.rs`

Tests the `WorkRepo` adapter against a real embedded SurrealKV instance.

```bash
cargo test --test '*' surreal_work_repo
```

**Test scenarios**:

```rust
#[tokio::test]
async fn test_create_and_retrieve_work() {
    // Setup: Create isolated SurrealDB instance for this test
    let db = create_test_db().await;
    
    // Test: Create a work
    let work = Work { id: uuid(), title: "Test Book", ... };
    let repo = SurrealWorkRepo::new(db.clone());
    repo.save_work(work.clone()).await.unwrap();
    
    // Verify: Retrieve and compare
    let retrieved = repo.find_by_id(&work.id).await.unwrap();
    assert_eq!(retrieved, work);
}
```

**Key test scenarios**:

1. **Create and Retrieve**
   - Create a work in SurrealDB
   - Retrieve by ID
   - Verify all fields match

2. **Deduplication (ISBN uniqueness)**
   - Create work with ISBN
   - Attempt to create duplicate ISBN
   - Expect `UniqueConstraint` error

3. **Status Transitions**
   - Create work with `pending` status
   - Update to `processing`
   - Update to `done`
   - Verify audit trail via `updated_at`

4. **Error Scenarios**
   - Query non-existent work → 404
   - Invalid ISBN format → Validation error
   - Database connection loss → Transient error

### NATS Integration Tests

**File**: `tests/integration/nats_consumer.rs`

Tests the NATS JetStream consumer and publisher.

```bash
cargo test --test '*' nats_consumer
```

**Test scenarios**:

1. **Publish and Consume Message**
   - Publisher creates message
   - Consumer receives and processes
   - Verify ACK is sent

2. **Retry Logic**
   - Consumer fails to process (nack)
   - Message is redelivered
   - Verify delivery count increments

3. **Dead Letter Queue**
   - Message fails after max retries
   - Verify it's moved to failed queue
   - Verify work status is set to `failed`

### External API Mock Tests

**File**: `tests/integration/gemini_mock.rs`

Uses the `mockito` crate to mock HTTP responses from Gemini API.

```bash
cargo test --test '*' gemini_mock
```

**Test scenarios**:

1. **Successful Concept Extraction**
   - Mock Gemini response with valid JSON schema
   - Verify adapter correctly deserializes response
   - Verify all fields are present

2. **Schema Violations (Missing Optional Fields)**
   - Mock response missing `strength` field
   - Adapter applies default (0.5)
   - Verify work still succeeds

3. **Schema Violations (Missing Required Fields)**
   - Mock response missing `name` field
   - Adapter treats as permanent failure
   - Verify work is marked as `failed`

4. **API Timeout**
   - Mock slow response (> 15s)
   - Adapter times out
   - Verify classified as **Transient** error

5. **Network Error**
   - Mock connection refused
   - Adapter gets `std::io::Error`
   - Verify classified as **Transient** error

### Running a Single Integration Test with Full Output

```bash
# Run one test with backtrace and logging
RUST_BACKTRACE=1 RUST_LOG=debug cargo test --test surreal_work_repo test_create_and_retrieve -- --nocapture
```

---

## Manual Testing

Manual testing validates the application through the user interface and API endpoints.

### Setup for Manual Testing

#### 1. Start the Application

```bash
# Set test environment variables
export KV_JWT_SECRET="test-secret-min-32-chars-for-testing"
export KV_GEMINI_API_KEY="AIzaSyD..."  # Your actual API key
export KV_DATA_DIR="./test-data"
export KV_PORT="8080"

# Build and run (dev mode with hot reload)
cargo leptos serve
```

The application will:
1. Extract NATS binary
2. Initialize SurrealDB
3. Start HTTP server on `http://localhost:8080`
4. Open browser automatically (or navigate to `http://localhost:8080`)

#### 2. Health Check

Before testing, verify the service is healthy:

```bash
curl http://localhost:8080/health

# Expected response:
{
  "status": "ok",
  "version": "0.1.0",
  "db": "connected"
}
```

### Test Scenarios

#### Scenario 1: First-Run Setup

**Steps**:

1. Navigate to `http://localhost:8080/`
2. Verify redirect to `/setup` page
3. Enter email: `admin@example.com`
4. Enter password: `TestPassword123!` (12+ chars)
5. Click "Create Account"
6. Verify redirect to `/login` page

**Expected Outcome**:
- Account created
- User can proceed to login

**Error Cases**:
- Password < 12 chars: Show "Password must be 12+ characters"
- Email already exists: Show "Account already created"

---

#### Scenario 2: Login and Token Generation

**Steps**:

1. Navigate to `/login`
2. Enter email: `admin@example.com`
3. Enter password: `TestPassword123!`
4. Click "Login"
5. Verify redirect to `/` (library page)
6. Verify session cookie is set

**Expected Outcome**:
- JWT token issued
- Cookie set with `HttpOnly`, `Secure`, `SameSite=Strict` flags
- User is authenticated for subsequent requests

**Error Cases**:
- Wrong password: Show "Invalid email or password", retry allowed
- After 3 failed attempts: Show "Too many login attempts, try again in 5 minutes"

---

#### Scenario 3: Personal Access Token (PAT) Creation

**Steps**:

1. Login as admin
2. Navigate to `/settings` (if implemented) or use API directly
3. Create PAT:
   ```bash
   curl -X POST http://localhost:8080/api/tokens \
     -H "Authorization: Bearer $JWT_TOKEN" \
     -H "Content-Type: application/json" \
     -d '{"name": "test-token"}'
   ```
4. Copy token (shown only once)
5. Verify token can be used for authentication:
   ```bash
   curl -H "Authorization: Bearer ens_..." http://localhost:8080/api/works
   ```

**Expected Outcome**:
- Token created with `ens_` prefix
- Can be used in place of JWT for API calls
- Token is hashed in database (raw value never stored)

---

#### Scenario 4: Submit Book (ISBN)

**Steps**:

1. On library page, click "Add Book" button
2. Enter ISBN: `9780134685991` (Clean Code by Robert C. Martin)
3. Click "Submit"
4. Verify work appears in library with "Pending" status

**Expected Outcome**:
- Work created with `pending` status
- UUID is assigned
- Work visible in works list

**Error Cases**:
- Invalid ISBN format: Show "Invalid ISBN format"
- Duplicate ISBN: Show "This book is already in your library"
- ISBN not found in Open Library: Show "Could not find book metadata"

---

#### Scenario 5: Async Processing (Book to Concepts)

**Steps**:

1. After submitting a book, watch the status in real-time
2. Observe status transitions:
   - `pending` → `processing` (fetching metadata)
   - `processing` → `done` (concepts extracted)
3. Click on the work to view details
4. Verify concepts are displayed with descriptions

**Expected Outcome**:
- Book metadata fetched from Open Library
- Gemini extracts concepts (takes 5-15 seconds)
- Concepts stored in graph with relationships
- Work status updated to `done`
- Insight (summary) displayed in detail view

**Real-time Monitoring** (WebSocket):

```bash
# In separate terminal, connect to WebSocket
curl -i -N -H "Connection: Upgrade" \
  -H "Upgrade: websocket" \
  -H "Sec-WebSocket-Key: x3JJHMbDL1EzLkh9GBhXDw==" \
  -H "Sec-WebSocket-Version: 13" \
  -H "Authorization: Bearer $JWT_TOKEN" \
  http://localhost:8080/ws

# You should see real-time status updates as a book is processed
```

---

#### Scenario 6: Concept Graph Visualization

**Steps**:

1. Add 3-5 books to build a concept graph
2. Navigate to `/graph` page
3. Verify graph displays with:
   - Nodes (concepts) in different colors by domain
   - Edges (relationships) with varying thickness
   - Pan/zoom/drag functionality
4. Click on a node to view concept details
5. Click on edge to see relationship type and strength

**Expected Outcome**:
- Graph renders correctly with Cytoscape.js
- Interactive controls work (zoom, pan, drag)
- Hover shows concept names
- Click navigates to concept detail

**Edge Cases**:
- Empty graph (no books): Show placeholder message
- Large graph (50+ books): Verify performance (< 1s load)

---

#### Scenario 7: Domain Filter

**Steps**:

1. On graph page, select domain filter (e.g., "Economics")
2. Verify graph updates to show only concepts in selected domain
3. Select multiple domains
4. Verify graph shows concepts in all selected domains

**Expected Outcome**:
- Graph filters dynamically without page reload
- Edge count decreases as irrelevant concepts are hidden
- Filter state persists in URL query parameters

---

#### Scenario 8: API Testing (with curl)

**Test authorization**:

```bash
# 1. Create user via setup
curl -X POST http://localhost:8080/setup \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "email=admin@example.com&password=TestPassword123"

# 2. Login
TOKEN=$(curl -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"admin@example.com","password":"TestPassword123"}' \
  | jq -r '.token')

# 3. List works (authenticated)
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/works | jq

# 4. Submit a work
curl -X POST http://localhost:8080/api/works \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "identifier": "9780134685991",
    "identifier_type": "isbn"
  }' | jq

# 5. Get work details
WORK_ID="..." # from previous response
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/works/$WORK_ID | jq

# 6. List concepts and relationships
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/graph | jq
```

---

## End-to-End Testing

E2E testing validates complete user journeys using the Minimum Viable Feature (MVF) checklist.

### MVF Test Checklist

Complete this checklist on a fresh installation:

**Setup (2 minutes)**:

- [ ] Download binary to clean environment
- [ ] Set required environment variables
- [ ] Start service
- [ ] Verify `/health` returns `ok`

**First-Run Setup (2 minutes)**:

- [ ] Navigate to `http://localhost:8080/`
- [ ] Create admin account with email & password
- [ ] Verify redirect to login page
- [ ] Login with credentials
- [ ] Verify redirect to library (empty state)

**Add 5 Books (10-15 minutes)**:

1. **Clean Code** (ISBN: 9780134685991)
   - [ ] Submit ISBN
   - [ ] Status changes to `processing`
   - [ ] Wait for Gemini (5-15 seconds)
   - [ ] Status changes to `done`
   - [ ] Concepts extracted and visible

2. **Refactoring** (ISBN: 9780201485677)
   - [ ] Submit ISBN
   - [ ] Wait for processing
   - [ ] Verify concepts extracted

3. **The Pragmatic Programmer** (ISBN: 9780135957059)
   - [ ] Submit ISBN
   - [ ] Wait for processing
   - [ ] Verify concepts extracted

4. **Design Patterns** (ISBN: 9780201633610)
   - [ ] Submit ISBN
   - [ ] Wait for processing
   - [ ] Verify concepts extracted

5. **Effective Java** (ISBN: 9780134685991)
   - [ ] Submit ISBN
   - [ ] Wait for processing
   - [ ] Verify concepts extracted

**Graph Visualization (5 minutes)**:

- [ ] Navigate to `/graph`
- [ ] Verify graph renders with all concepts
- [ ] Zoom in/out works
- [ ] Pan works (drag background)
- [ ] Click node shows concept details
- [ ] Click node navigates to concept detail page

**API Verification (5 minutes)**:

```bash
# [ ] Create PAT and use for API calls
# [ ] GET /api/works returns all 5 books
# [ ] GET /api/graph returns nodes and edges
# [ ] Concept count > 50
# [ ] Edges connect related concepts
```

**Total Time**: 25-35 minutes on a fresh installation

---

## Performance Testing

Performance tests verify that the application meets its NFR budgets.

### Tools

```bash
# Install Apache Bench
sudo apt-get install apache2-utils

# Install wrk (modern load testing)
brew install wrk  # macOS
# or https://github.com/wg/wrk for Linux
```

### Test 1: Page Load Time (SSR)

```bash
# Measure time to first byte for library page (cached)
ab -n 100 -c 10 http://localhost:8080/

# Expected: Median < 50ms (SSR + Leptos hydration)
```

### Test 2: API Response Time

```bash
# Test graph endpoint with 5+ books
curl -w "@curl-format.txt" -o /dev/null -s \
  -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/graph

# Expected: < 100ms for concept traversal (depth 3)
```

### Test 3: Binary Size

```bash
# Check stripped binary size
ls -lh /usr/local/bin/knowledge-vault

# Expected: < 60 MB (NFR-03)
```

### Test 4: Cold Start Time

```bash
# Measure startup time
time /usr/local/bin/knowledge-vault --help

# Expected: < 2 seconds (NFR-04)
```

### Test 5: Memory Usage

```bash
# Monitor memory during processing
while true; do
  ps aux | grep knowledge-vault | grep -v grep | awk '{print $6}'
  sleep 1
done

# Expected: < 512 MB during normal operation
```

---

## Security Testing

Security tests verify that authentication, authorization, and data protection work correctly.

### Test 1: Authentication

```bash
# Test missing Authorization header
curl -X POST http://localhost:8080/api/works \
  -H "Content-Type: application/json" \
  -d '{...}'
# Expected: 401 Unauthorized

# Test invalid token
curl -H "Authorization: Bearer invalid-token" \
  http://localhost:8080/api/works
# Expected: 401 Unauthorized

# Test expired token (after 24 hours)
# Expected: 401 Unauthorized
```

### Test 2: PAT Revocation

```bash
# Create PAT
PAT=$(curl -X POST http://localhost:8080/api/tokens \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"test"}' | jq -r '.token')

# Use PAT (should work)
curl -H "Authorization: Bearer $PAT" \
  http://localhost:8080/api/works
# Expected: 200 OK

# Revoke PAT
TOKEN_ID="..."
curl -X DELETE http://localhost:8080/api/tokens/$TOKEN_ID \
  -H "Authorization: Bearer $JWT_TOKEN"

# Use revoked PAT (should fail)
curl -H "Authorization: Bearer $PAT" \
  http://localhost:8080/api/works
# Expected: 401 Unauthorized
```

### Test 3: Login Rate Limiting

```bash
# Attempt login 3 times with wrong password
for i in {1..3}; do
  curl -X POST http://localhost:8080/auth/login \
    -H "Content-Type: application/json" \
    -d '{"email":"admin@example.com","password":"wrong"}'
done

# 4th attempt should be rate-limited
curl -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"admin@example.com","password":"wrong"}'
# Expected: 429 Too Many Requests
```

### Test 4: CORS (should be disabled)

```bash
# Send CORS preflight request
curl -X OPTIONS http://localhost:8080/api/works \
  -H "Origin: https://evil.com" \
  -H "Access-Control-Request-Method: GET" \
  -v

# Expected: No CORS headers in response (CORS disabled by design)
```

### Test 5: SQL Injection (SurrealDB parameterized queries)

```bash
# Try to inject SurrealQL via ISBN
curl -X POST http://localhost:8080/api/works \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "identifier": "9780134685991; DROP TABLE work;",
    "identifier_type": "isbn"
  }'

# Expected: Invalid ISBN format error (parameterized queries prevent injection)
```

### Test 6: XSS Prevention

```bash
# Try to submit book with XSS payload
curl -X POST http://localhost:8080/api/works \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "<img src=x onerror='alert(1)'>",
    "identifier_type": "manual"
  }'

# Verify in browser: payload should be HTML-escaped (Leptos output-encodes)
```

---

## Test Data Setup

### Creating Test Users

```bash
# Via API
curl -X POST http://localhost:8080/setup \
  -H "Content-Type: application/json" \
  -d '{"email":"test@example.com","password":"TestPassword123"}'

# Multiple users (requires code changes for v2)
```

### Creating Test Books

```bash
# Fixture: Known ISBNs with reliable Open Library data

TEST_BOOKS=(
  "9780134685991"  # Clean Code
  "9780201633610"  # Design Patterns
  "9780135957059"  # The Pragmatic Programmer
  "9780201485677"  # Refactoring
  "9780134494166"  # Java Concurrency in Practice
)

for ISBN in "${TEST_BOOKS[@]}"; do
  curl -X POST http://localhost:8080/api/works \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d "{\"identifier\": \"$ISBN\", \"identifier_type\": \"isbn\"}"
  
  # Wait for processing (check status)
  sleep 20
done
```

### Resetting Test Data

```bash
# Stop service
sudo systemctl stop knowledge-vault

# Remove data directory
sudo rm -rf /var/lib/knowledge-vault

# Restart service
sudo systemctl start knowledge-vault
```

---

## Continuous Testing (CI/CD)

The GitHub Actions workflow (`.github/workflows/ci.yml`) runs automated tests on every commit:

1. **Lint**: `cargo clippy`
2. **Format check**: `cargo fmt -- --check`
3. **Unit tests**: `cargo test --lib`
4. **Integration tests**: `cargo test --test '*'`
5. **Security audit**: `cargo audit`
6. **Build**: `cargo build --release --target aarch64-unknown-linux-musl`
7. **Binary size check**: Verify < 60 MB

All tests must pass before merge to `main`.

---

*End of Testing Guide v1.0 — Knowledge Vault*
