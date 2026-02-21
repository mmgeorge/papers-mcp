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

1. The API was explored via live `curl` calls against a real user library
2. Every endpoint was tested to verify response shapes, header behavior, and pagination
3. Results were codified into `api-spec.toml` as documentation
4. Type structs were derived from actual JSON responses

## Architecture

- `api-spec.toml` — Documented API specification: endpoints, parameters, response types
- `src/client.rs` — `ZoteroClient` struct with 25+ public methods (one per endpoint)
- `src/types/` — Serde-deserializable Rust structs for items, collections, tags, searches, groups
- `src/params.rs` — Parameter structs with `#[derive(Default, bon::Builder)]`
- `src/response.rs` — `PagedResponse<T>` wrapper combining array body + header metadata
- `src/cache.rs` — `DiskCache` identical to papers-openalex
- `src/error.rs` — Error types for HTTP, JSON, and API errors
- `tests/fixtures/` — JSON response fixtures captured from the live API

## Entity Types

| Entity | List | Get | Type file |
|------------|------|-----|-----------|
| Item | Yes (8 endpoints) | Yes | `types/item.rs` |
| Collection | Yes (3 endpoints) | Yes | `types/collection.rs` |
| Tag | Yes (10 endpoints) | Yes (returns array) | `types/tag.rs` |
| SavedSearch | Yes | Yes | `types/search.rs` |
| Group | Yes | No | `types/group.rs` |

## How to Update When the API Changes

### Step 1: Check for changes
Visit https://www.zotero.org/support/dev/web_api/v3/start and compare against `api-spec.toml`.

### Step 2: Verify against live API
```
curl -s -H "Zotero-API-Version: 3" -H "Zotero-API-Key: $KEY" \
  "https://api.zotero.org/users/16916553/items?limit=1" -D -
```

### Step 3: Update Rust code
- New/changed entity fields → update structs in `src/types/*.rs`
- New endpoints → add methods to `src/client.rs`
- New parameters → update structs in `src/params.rs`

### Step 4: Update tests
- Add/update fixtures in `tests/fixtures/`
- Add wiremock tests in `src/client.rs`
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

- **Raw arrays:** Zotero returns `[...]` not `{results: [...]}`. List endpoints parse the body as a `Vec<T>`
- **Pagination in headers:** `Total-Results` and `Last-Modified-Version` are HTTP response headers, not body fields
- **`parentCollection` is `false`:** Top-level collections have `"parentCollection": false` (JSON boolean), not `null`. Use `serde_json::Value` to handle this
- **Auth headers required:** Every request must include `Zotero-API-Version: 3` and `Zotero-API-Key: <key>` headers
- **User-scoped paths:** All endpoints are prefixed with `/users/<id>` (or `/groups/<id>` for group libraries)
- **Item type variance:** Different item types (journalArticle, book, attachment, note) have different fields. `ItemData` uses `#[serde(flatten)]` for extra fields
- **Tag `get_tag` returns array:** `GET /tags/<name>` returns a JSON array (usually with 1 element), not a single object
- **`publications/items/tags` quirk:** Returns ALL library tags, not just publication tags
- **Creator formats:** Creators use either `firstName`+`lastName` or a single `name` field (institutional authors)
- **`format=keys`:** Returns newline-separated plain text, not JSON
- **`format=versions`:** Returns `{key: version}` JSON object, not array
- **Tag `Total-Results` is 0:** The tags endpoint returns `Total-Results: 0` in the header even when the body contains results. Do not rely on `total_results` for tag counts
- **Cache stores headers:** Because pagination info is in headers, the cache wraps body + header metadata together
