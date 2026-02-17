use serde::{Deserialize, Serialize};

/// Paginated response wrapping Zotero array results with header metadata.
///
/// Zotero API responses are raw JSON arrays `[...]` with pagination info in
/// HTTP headers (`Total-Results`, `Last-Modified-Version`). This struct
/// combines both into a single type.
///
/// # Example
///
/// ```no_run
/// # async fn example() -> papers_zotero::Result<()> {
/// use papers_zotero::{ZoteroClient, ItemListParams};
///
/// let client = ZoteroClient::from_env()?;
/// let resp = client.list_items(&ItemListParams::default()).await?;
/// println!("Total: {:?}, got: {}", resp.total_results, resp.items.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagedResponse<T> {
    /// The array of results from the response body.
    pub items: Vec<T>,

    /// Total number of results available (from `Total-Results` header).
    /// `None` if the header was absent (e.g. single-entity endpoints).
    pub total_results: Option<u64>,

    /// Library version (from `Last-Modified-Version` header). Used for
    /// incremental sync via the `since` parameter.
    pub last_modified_version: Option<u64>,
}
