//! Async Rust client for the [Zotero Web API v3](https://www.zotero.org/support/dev/web_api/v3/start).
//!
//! Zotero is a personal research library manager. This crate provides a
//! type-safe async client for reading items, collections, tags, searches,
//! and groups from a Zotero user or group library.
//!
//! # Quick start
//!
//! ```no_run
//! # async fn example() -> papers_zotero::Result<()> {
//! use papers_zotero::{ZoteroClient, ItemListParams};
//!
//! let client = ZoteroClient::new("16916553", "your-api-key");
//!
//! // Search for items about rendering
//! let params = ItemListParams::builder()
//!     .q("rendering")
//!     .limit(5)
//!     .build();
//! let response = client.list_items(&params).await?;
//! println!("Found {:?} items", response.total_results);
//! for item in &response.items {
//!     println!("  - {}", item.data.title.as_deref().unwrap_or("untitled"));
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Authentication
//!
//! A Zotero API key is required. Create one at
//! <https://www.zotero.org/settings/keys>. Pass it explicitly or set the
//! `ZOTERO_API_KEY` and `ZOTERO_USER_ID` environment variables:
//!
//! ```no_run
//! use papers_zotero::ZoteroClient;
//!
//! // Explicit credentials
//! let client = ZoteroClient::new("16916553", "your-key");
//!
//! // Or from environment
//! let client = ZoteroClient::from_env().unwrap();
//! ```
//!
//! # Endpoints
//!
//! The client provides 40+ methods covering all Zotero read and write endpoints:
//!
//! **Read:**
//! - **9 item endpoints** — list/get items, top items, trash, children,
//!   collection items, publication items, file download, file view, file URL
//! - **4 collection endpoints** — list/get collections, top, subcollections
//! - **10 tag endpoints** — list tags across various scopes (items, collections,
//!   trash, publications)
//! - **2 search endpoints** — list/get saved searches
//! - **2 full-text endpoints** — list indexed versions, get item full-text
//! - **1 deleted endpoint** — get deleted object keys since a version
//! - **2 settings endpoints** — get all settings, get single setting
//! - **1 group endpoint** — list user groups
//! - **2 key endpoints** — get API key info by value or by current request
//!
//! **Write:**
//! - **5 item write endpoints** — create, update (PUT), patch (PATCH),
//!   delete single, delete multiple
//! - **4 collection write endpoints** — create, update, delete single,
//!   delete multiple
//! - **2 search write endpoints** — create, delete multiple
//! - **1 tag write endpoint** — delete multiple tags

pub mod cache;
pub mod client;
pub mod error;
pub mod params;
pub mod response;
pub mod types;

pub use cache::DiskCache;
pub use client::ZoteroClient;
pub use error::{Result, ZoteroError};
pub use params::{CollectionListParams, DeletedParams, FulltextParams, ItemListParams, TagListParams};
pub use response::{PagedResponse, VersionedResponse};
pub use types::*;
