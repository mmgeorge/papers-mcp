use serde::{Deserialize, Serialize};

/// Response wrapping a single Zotero object with its sync version header.
///
/// Used by endpoints that return one JSON object (not an array) and include
/// a `Last-Modified-Version` response header — e.g. fulltext, deleted objects,
/// and settings.
///
/// # Example
///
/// ```no_run
/// # async fn example() -> papers_zotero::Result<()> {
/// use papers_zotero::{ZoteroClient, DeletedParams};
///
/// let client = ZoteroClient::from_env()?;
/// let resp = client.get_deleted(&DeletedParams::builder().since(0u64).build()).await?;
/// println!("Deleted {} items as of version {:?}", resp.data.items.len(), resp.last_modified_version);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedResponse<T> {
    /// The deserialized response body.
    pub data: T,

    /// Library version at the time of the response (from `Last-Modified-Version`
    /// header). Use this as the `since` value for subsequent sync calls.
    pub last_modified_version: Option<u64>,
}

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
