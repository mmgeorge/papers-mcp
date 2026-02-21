# Zotero Rust Client — Agent Guide

## Origin

This crate is a Rust API client for the [Zotero Web API v3](https://www.zotero.org/support/dev/web_api/v3/start). Zotero is a personal research library manager for collecting, organizing, and citing research papers.

## Zotero Rest Documentation
- Start here (index): https://www.zotero.org/support/dev/web_api/v3/start
- Basics & read requests: https://www.zotero.org/support/dev/web_api/v3/basics
- Write requests: https://www.zotero.org/support/dev/web_api/v3/write_requests
- File uploads/downloads: https://www.zotero.org/support/dev/web_api/v3/file_upload
- Full-text content: https://www.zotero.org/support/dev/web_api/v3/fulltext_content
- Syncing: https://www.zotero.org/support/dev/web_api/v3/syncing
- Streaming API: https://www.zotero.org/support/dev/web_api/v3/streaming_api

## How This Crate Was Derived

1. The API was explored via live calls against a real user library
2. Every endpoint was tested to verify response shapes, header behavior, and pagination
3. Results were codified into `api-spec.toml` as documentation
4. Type structs were derived from actual JSON responses

## Architecture

- `api-spec.toml` — Documented API specification: endpoints, parameters, response types (read and write)
- `src/client.rs` — `ZoteroClient` struct with 40+ public methods (one per endpoint)
- `src/types/` — Serde-deserializable Rust structs for all response and write types
- `src/params.rs` — Parameter structs with `#[derive(Default, bon::Builder)]`
- `src/response.rs` — `PagedResponse<T>` and `VersionedResponse<T>` wrappers
- `src/cache.rs` — `DiskCache` identical to papers-openalex
- `src/error.rs` — Error types for HTTP, JSON, and API errors
- `tests/fixtures/` — JSON response fixtures captured from the live API

## Entity Types

| Entity | List | Get | Write | Type file |
|------------|------|-----|-------|-----------|
| Item | Yes (8 endpoints) | Yes | create, update, patch, delete | `types/item.rs` |
| Collection | Yes (3 endpoints) | Yes | create, update, delete | `types/collection.rs` |
| Tag | Yes (10 endpoints) | Yes (returns array) | delete | `types/tag.rs` |
| SavedSearch | Yes | Yes | create, delete | `types/search.rs` |
| Group | Yes | No | — | `types/group.rs` |

Write operations return `WriteResponse` (creates) or `()` (updates/deletes). See `types/write.rs`.

## Credentials & Environment Variables

| Variable | Purpose |
|---|---|
| `ZOTERO_USER_ID` | Production library user ID |
| `ZOTERO_API_KEY` | Production library API key |
| `ZOTERO_TEST_USER_ID` | Dedicated test library user ID (used by integration tests and scripts) |
| `ZOTERO_TEST_API_KEY` | Dedicated test library API key (write access) |

Integration tests (`tests/integration.rs`) always use `ZOTERO_TEST_USER_ID` / `ZOTERO_TEST_API_KEY`.
The production key (`ZOTERO_API_KEY`) is never used in tests.

## Scripts

Scripts live in `scripts/` at the repo root:

- **`zotero.ps1`** — GET explorer. Pass a path relative to `/users/<id>` and it is called against the test library. E.g.: `.\scripts\zotero.ps1 /items/top?limit=2`
- **`zotero-write.ps1`** — End-to-end write test. Creates items, collections, and a saved search; patches/PUTs some of them; then deletes everything and verifies the library is empty.

Both scripts read `ZOTERO_TEST_API_KEY` and `ZOTERO_TEST_USER_ID` from the user environment.

## How to Update When the API Changes

### Step 1: Check for changes
Visit https://www.zotero.org/support/dev/web_api/v3/start and compare against `api-spec.toml`.

### Step 2: Verify against live API
Use the `zotero.ps1` script:
```powershell
.\scripts\zotero.ps1 /items?limit=1
```

### Step 3: Update Rust code
- New/changed entity fields → update structs in `src/types/*.rs`
- New read endpoints → add methods to `src/client.rs` using existing `get_json_array` / `get_json_single` / `get_json_versioned` helpers
- New write endpoints → add methods using `post_json_write` / `put_no_content` / `patch_no_content` / `delete_no_content` / `delete_multiple_no_content` helpers
- New parameters → update structs in `src/params.rs`

### Step 4: Update tests
- Add/update fixtures in `tests/fixtures/`
- Add wiremock unit tests in `src/client.rs`
- Add live integration tests in `tests/integration.rs`

## Error Types

`ZoteroError` (in `src/error.rs`) has four variants:
- `Http` — network/connection failure (wraps `reqwest::Error`)
- `Json` — deserialization failure (wraps `serde_json::Error`)
- `Api { status, message }` — non-success HTTP status from the server
- `NotRunning { path: String }` — Zotero is installed on disk but its local API is unreachable. Only returned by `from_env_prefer_local`. The `path` field is the filesystem path where the Zotero executable was found.

## Install Detection

`find_zotero_exe()` (private, in `client.rs`) checks platform-specific install paths:
- **Windows**: `%PROGRAMFILES%\Zotero\zotero.exe` and `%LOCALAPPDATA%\Zotero\zotero.exe`
- **macOS**: `/Applications/Zotero.app/Contents/MacOS/zotero`
- **Linux**: `$HOME/Zotero/zotero`, `/opt/Zotero/zotero`, `/usr/lib/zotero/zotero`, `/usr/bin/zotero`

Returns `None` if none of the candidates exist on disk.

## `ZOTERO_CHECK_LAUNCHED` Environment Variable

When `from_env_prefer_local` fails to reach the local API, it calls `find_zotero_exe()`.
If an executable is found, it returns `Err(ZoteroError::NotRunning { path })` — an actionable error telling the user to start Zotero.

Set `ZOTERO_CHECK_LAUNCHED=0` to disable this check and fall back silently to the remote web API (useful for CI or non-interactive contexts where Zotero is not expected to be running).

## Key Gotchas

### Read
- **Raw arrays:** Zotero returns `[...]` not `{results: [...]}`. List endpoints parse the body as a `Vec<T>`
- **Pagination in headers:** `Total-Results` and `Last-Modified-Version` are HTTP response headers, not body fields
- **`parentCollection` is `false`:** Top-level collections have `"parentCollection": false` (JSON boolean), not `null`. Use `serde_json::Value` to handle this
- **Tag `get_tag` returns array:** `GET /tags/<name>` returns a JSON array (usually with 1 element), not a single object
- **`publications/items/tags` quirk:** Returns ALL library tags, not just publication tags
- **Creator formats:** Creators use either `firstName`+`lastName` or a single `name` field (institutional authors)
- **`format=keys`:** Returns newline-separated plain text, not JSON
- **`format=versions`:** Returns `{key: version}` JSON object, not array
- **Tag `Total-Results` is 0:** The tags endpoint returns `Total-Results: 0` in the header even when the body contains results. Do not rely on `total_results` for tag counts
- **Cache stores headers:** Because pagination info is in headers, the cache wraps body + header metadata together

### Write
- **Auth headers required:** Every request must include `Zotero-API-Version: 3` and `Zotero-API-Key: <key>`
- **`If-Unmodified-Since-Version`:** Required on PUT, PATCH, and DELETE. For single-object operations pass the item/collection version; for multi-delete pass the library version. Returns `412 Precondition Failed` if stale.
- **`WriteResponse` index keys are strings:** `successful["0"]`, `successful["1"]`, etc. — not integers
- **Creates return full objects:** `successful` values are complete saved objects with server-assigned keys and versions
- **Max 50 objects per POST:** The API rejects batches larger than 50 items
- **Tag delete encoding:** Tags are URL-encoded and joined with ` || ` (space-pipe-pipe-space) in the query parameter
