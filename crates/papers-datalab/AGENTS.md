# papers-datalab

Async Rust client for the [DataLab Marker REST API](https://www.datalab.to).
Submits PDF bytes, polls until conversion is complete, and returns structured
output (markdown, JSON, images).

## Architecture

```
src/
  lib.rs      — pub re-exports
  client.rs   — DatalabClient (submit_marker, get_marker_result, convert_document)
  types.rs    — request/response types
  error.rs    — DatalabError
```

### DatalabClient

Two low-level methods (`submit_marker`, `get_marker_result`) plus one high-level
convenience method (`convert_document`) that submits and polls in a loop.

`with_base_url(url)` overrides the API base — use this in tests to point at a
mock server.

## Testing DataLab calls

**Always mock with wiremock — never call the real DataLab API from tests.**

Real API calls:
- Spend per-page credits
- Take ~25 seconds for a typical paper
- Require `DATALAB_API_KEY` in the environment

Use `DatalabClient::new("mock-key").with_base_url(server.uri())` and mount mocks
for the two endpoints the client calls:

| Endpoint | Purpose |
|---|---|
| `POST /api/v1/marker` | Submit document, returns `{ "request_id": "...", "success": true }` |
| `GET /api/v1/marker/{id}` | Poll result, returns `{ "status": "complete", "markdown": "...", ... }` |

Minimal wiremock setup:

```rust
let server = MockServer::start().await;

Mock::given(method("POST")).and(path("/api/v1/marker"))
    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "request_id": "test-req-1", "success": true
    })))
    .mount(&server).await;

Mock::given(method("GET")).and(path_regex(r"^/api/v1/marker/test-req-1$"))
    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "status": "complete", "success": true,
        "markdown": "# Test\n\nContent.",
        "json": {"pages": []}
    })))
    .mount(&server).await;

let client = DatalabClient::new("mock-key").with_base_url(server.uri());
```

See `papers-core/tests/text_cache.rs` → `setup_datalab_mock()` for the
canonical reusable helper.
