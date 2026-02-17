# papers

Monorepo for `papers` — a CLI and MCP server for searching academic research via OpenAlex.

[OpenAlex](https://openalex.org) is a fully open catalog of the global research system: over
240 million scholarly works, 90 million author profiles, and comprehensive metadata on journals,
institutions, research topics, publishers, and funders. It serves as a free, community-driven
alternative to proprietary databases like Scopus and Web of Science.

## Crates

| Crate | Description |
|-------|-------------|
| [`openalex`](crates/openalex/) | Typed Rust client for the OpenAlex REST API. Handles pagination, autocomplete, and semantic search. |
| [`papers`](crates/papers/) | Shared library consumed by both the MCP server and CLI. Wraps `openalex` with concise list responses that omit large fields like `referenced_works` and `counts_by_year`. |
| [`papers-mcp`](crates/papers-mcp/) | MCP server exposing 22 tools for searching and retrieving entities. Works with Claude Desktop, Claude Code, and any MCP-compatible client. |
| [`papers-cli`](crates/papers-cli/) | CLI binary (`papers`). The same 22 operations as the MCP server, with human-readable or JSON output. |

## Build and test

```sh
cargo build --workspace
cargo test --workspace
```
