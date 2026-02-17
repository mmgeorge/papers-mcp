use crate::cache::DiskCache;
use crate::error::{Result, ZoteroError};
use crate::params::{CollectionListParams, ItemListParams, TagListParams};
use crate::response::PagedResponse;
use crate::types::*;
use serde::de::DeserializeOwned;

const DEFAULT_BASE_URL: &str = "https://api.zotero.org";

/// Async client for the [Zotero Web API v3](https://www.zotero.org/support/dev/web_api/v3/start).
///
/// Provides 25+ methods covering all read endpoints for items, collections,
/// tags, searches, and groups in a user's Zotero library.
///
/// # Creating a client
///
/// ```no_run
/// use papers_zotero::ZoteroClient;
///
/// // Read credentials from ZOTERO_USER_ID and ZOTERO_API_KEY env vars
/// let client = ZoteroClient::from_env().unwrap();
///
/// // Or pass explicit credentials
/// let client = ZoteroClient::new("16916553", "your-api-key");
/// ```
///
/// # Example: list items
///
/// ```no_run
/// # async fn example() -> papers_zotero::Result<()> {
/// use papers_zotero::{ZoteroClient, ItemListParams};
///
/// let client = ZoteroClient::from_env()?;
/// let params = ItemListParams::builder()
///     .q("machine learning")
///     .sort("dateModified")
///     .direction("desc")
///     .limit(5)
///     .build();
/// let response = client.list_items(&params).await?;
/// println!("Total: {:?}, got: {}", response.total_results, response.items.len());
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct ZoteroClient {
    http: reqwest::Client,
    base_url: String,
    user_id: String,
    api_key: String,
    cache: Option<DiskCache>,
}

impl ZoteroClient {
    /// Create a new client with explicit user ID and API key.
    pub fn new(user_id: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: DEFAULT_BASE_URL.to_string(),
            user_id: user_id.into(),
            api_key: api_key.into(),
            cache: None,
        }
    }

    /// Create a client from `ZOTERO_USER_ID` and `ZOTERO_API_KEY` environment
    /// variables.
    ///
    /// Returns `Err` if either variable is not set.
    pub fn from_env() -> Result<Self> {
        let user_id = std::env::var("ZOTERO_USER_ID").map_err(|_| ZoteroError::Api {
            status: 0,
            message: "ZOTERO_USER_ID environment variable not set".into(),
        })?;
        let api_key = std::env::var("ZOTERO_API_KEY").map_err(|_| ZoteroError::Api {
            status: 0,
            message: "ZOTERO_API_KEY environment variable not set".into(),
        })?;
        Ok(Self::new(user_id, api_key))
    }

    /// Override the base URL. Useful for testing with a mock server.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Enable disk caching of successful responses.
    pub fn with_cache(mut self, cache: DiskCache) -> Self {
        self.cache = Some(cache);
        self
    }

    // ── Private helpers ────────────────────────────────────────────────

    fn user_prefix(&self) -> String {
        format!("/users/{}", self.user_id)
    }

    /// GET request returning a JSON array with header-based pagination.
    async fn get_json_array<T: DeserializeOwned>(
        &self,
        path: &str,
        query: Vec<(&str, String)>,
    ) -> Result<PagedResponse<T>> {
        let url = format!("{}{}", self.base_url, path);
        if let Some(cache) = &self.cache
            && let Some(text) = cache.get(&url, &query, None)
        {
            // Cached responses store body + header metadata as JSON
            let cached: CachedArrayResponse =
                serde_json::from_str(&text).map_err(ZoteroError::Json)?;
            let items: Vec<T> =
                serde_json::from_str(&cached.body).map_err(ZoteroError::Json)?;
            return Ok(PagedResponse {
                items,
                total_results: cached.total_results,
                last_modified_version: cached.last_modified_version,
            });
        }
        let resp = self
            .http
            .get(&url)
            .query(&query)
            .header("Zotero-API-Version", "3")
            .header("Zotero-API-Key", &self.api_key)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(ZoteroError::Api {
                status: status.as_u16(),
                message,
            });
        }
        let total_results = resp
            .headers()
            .get("Total-Results")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok());
        let last_modified_version = resp
            .headers()
            .get("Last-Modified-Version")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok());
        let text = resp.text().await?;
        if let Some(cache) = &self.cache {
            let cached = CachedArrayResponse {
                body: text.clone(),
                total_results,
                last_modified_version,
            };
            if let Ok(cache_text) = serde_json::to_string(&cached) {
                cache.set(&url, &query, None, &cache_text);
            }
        }
        let items: Vec<T> = serde_json::from_str(&text).map_err(ZoteroError::Json)?;
        Ok(PagedResponse {
            items,
            total_results,
            last_modified_version,
        })
    }

    /// GET request returning a single JSON object.
    async fn get_json_single<T: DeserializeOwned>(
        &self,
        path: &str,
        query: Vec<(&str, String)>,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        if let Some(cache) = &self.cache
            && let Some(text) = cache.get(&url, &query, None)
        {
            return serde_json::from_str(&text).map_err(ZoteroError::Json);
        }
        let resp = self
            .http
            .get(&url)
            .query(&query)
            .header("Zotero-API-Version", "3")
            .header("Zotero-API-Key", &self.api_key)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(ZoteroError::Api {
                status: status.as_u16(),
                message,
            });
        }
        let text = resp.text().await?;
        if let Some(cache) = &self.cache {
            cache.set(&url, &query, None, &text);
        }
        serde_json::from_str(&text).map_err(ZoteroError::Json)
    }

    /// GET request returning raw bytes (for file downloads).
    /// Does not use caching (files are too large).
    async fn get_binary(&self, path: &str) -> Result<Vec<u8>> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .get(&url)
            .header("Zotero-API-Version", "3")
            .header("Zotero-API-Key", &self.api_key)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(ZoteroError::Api {
                status: status.as_u16(),
                message,
            });
        }
        Ok(resp.bytes().await?.to_vec())
    }

    // ── Item endpoints ─────────────────────────────────────────────────

    /// List all items in the library.
    ///
    /// `GET /users/<id>/items`
    pub async fn list_items(&self, params: &ItemListParams) -> Result<PagedResponse<Item>> {
        let path = format!("{}/items", self.user_prefix());
        self.get_json_array(&path, params.to_query_pairs()).await
    }

    /// List top-level items (excludes child attachments and notes).
    ///
    /// `GET /users/<id>/items/top`
    pub async fn list_top_items(&self, params: &ItemListParams) -> Result<PagedResponse<Item>> {
        let path = format!("{}/items/top", self.user_prefix());
        self.get_json_array(&path, params.to_query_pairs()).await
    }

    /// List items in the trash.
    ///
    /// `GET /users/<id>/items/trash`
    pub async fn list_trash_items(&self, params: &ItemListParams) -> Result<PagedResponse<Item>> {
        let path = format!("{}/items/trash", self.user_prefix());
        self.get_json_array(&path, params.to_query_pairs()).await
    }

    /// Get a single item by key.
    ///
    /// `GET /users/<id>/items/<key>`
    pub async fn get_item(&self, key: &str) -> Result<Item> {
        let path = format!("{}/items/{}", self.user_prefix(), key);
        self.get_json_single(&path, vec![]).await
    }

    /// List child items (attachments and notes) of a parent item.
    ///
    /// `GET /users/<id>/items/<key>/children`
    pub async fn list_item_children(
        &self,
        key: &str,
        params: &ItemListParams,
    ) -> Result<PagedResponse<Item>> {
        let path = format!("{}/items/{}/children", self.user_prefix(), key);
        self.get_json_array(&path, params.to_query_pairs()).await
    }

    /// List items in "My Publications".
    ///
    /// `GET /users/<id>/publications/items`
    pub async fn list_publication_items(
        &self,
        params: &ItemListParams,
    ) -> Result<PagedResponse<Item>> {
        let path = format!("{}/publications/items", self.user_prefix());
        self.get_json_array(&path, params.to_query_pairs()).await
    }

    /// List items in a specific collection.
    ///
    /// `GET /users/<id>/collections/<key>/items`
    pub async fn list_collection_items(
        &self,
        collection_key: &str,
        params: &ItemListParams,
    ) -> Result<PagedResponse<Item>> {
        let path = format!(
            "{}/collections/{}/items",
            self.user_prefix(),
            collection_key
        );
        self.get_json_array(&path, params.to_query_pairs()).await
    }

    /// List top-level items in a specific collection (excludes child
    /// attachments/notes).
    ///
    /// `GET /users/<id>/collections/<key>/items/top`
    pub async fn list_collection_top_items(
        &self,
        collection_key: &str,
        params: &ItemListParams,
    ) -> Result<PagedResponse<Item>> {
        let path = format!(
            "{}/collections/{}/items/top",
            self.user_prefix(),
            collection_key
        );
        self.get_json_array(&path, params.to_query_pairs()).await
    }

    /// Download the file content of an attachment item.
    ///
    /// `GET /users/<id>/items/<key>/file`
    ///
    /// Returns raw bytes. The reqwest client follows the S3 redirect
    /// automatically.
    pub async fn download_item_file(&self, key: &str) -> Result<Vec<u8>> {
        let path = format!("{}/items/{}/file", self.user_prefix(), key);
        self.get_binary(&path).await
    }

    // ── Collection endpoints ───────────────────────────────────────────

    /// List all collections in the library.
    ///
    /// `GET /users/<id>/collections`
    pub async fn list_collections(
        &self,
        params: &CollectionListParams,
    ) -> Result<PagedResponse<Collection>> {
        let path = format!("{}/collections", self.user_prefix());
        self.get_json_array(&path, params.to_query_pairs()).await
    }

    /// List top-level collections (no parent).
    ///
    /// `GET /users/<id>/collections/top`
    pub async fn list_top_collections(
        &self,
        params: &CollectionListParams,
    ) -> Result<PagedResponse<Collection>> {
        let path = format!("{}/collections/top", self.user_prefix());
        self.get_json_array(&path, params.to_query_pairs()).await
    }

    /// Get a single collection by key.
    ///
    /// `GET /users/<id>/collections/<key>`
    pub async fn get_collection(&self, key: &str) -> Result<Collection> {
        let path = format!("{}/collections/{}", self.user_prefix(), key);
        self.get_json_single(&path, vec![]).await
    }

    /// List sub-collections of a collection.
    ///
    /// `GET /users/<id>/collections/<key>/collections`
    pub async fn list_subcollections(
        &self,
        key: &str,
        params: &CollectionListParams,
    ) -> Result<PagedResponse<Collection>> {
        let path = format!("{}/collections/{}/collections", self.user_prefix(), key);
        self.get_json_array(&path, params.to_query_pairs()).await
    }

    // ── Search endpoints ───────────────────────────────────────────────

    /// List saved searches.
    ///
    /// `GET /users/<id>/searches`
    pub async fn list_searches(&self) -> Result<PagedResponse<SavedSearch>> {
        let path = format!("{}/searches", self.user_prefix());
        self.get_json_array(&path, vec![]).await
    }

    /// Get a single saved search by key.
    ///
    /// `GET /users/<id>/searches/<key>`
    pub async fn get_search(&self, key: &str) -> Result<SavedSearch> {
        let path = format!("{}/searches/{}", self.user_prefix(), key);
        self.get_json_single(&path, vec![]).await
    }

    // ── Tag endpoints ──────────────────────────────────────────────────

    /// List all tags in the library.
    ///
    /// `GET /users/<id>/tags`
    pub async fn list_tags(&self, params: &TagListParams) -> Result<PagedResponse<Tag>> {
        let path = format!("{}/tags", self.user_prefix());
        self.get_json_array(&path, params.to_query_pairs()).await
    }

    /// Get a single tag by name.
    ///
    /// `GET /users/<id>/tags/<urlencoded-name>`
    pub async fn get_tag(&self, name: &str) -> Result<PagedResponse<Tag>> {
        let encoded = urlencoded(name);
        let path = format!("{}/tags/{}", self.user_prefix(), encoded);
        self.get_json_array(&path, vec![]).await
    }

    /// List tags on a specific item.
    ///
    /// `GET /users/<id>/items/<key>/tags`
    pub async fn list_item_tags(
        &self,
        key: &str,
        params: &TagListParams,
    ) -> Result<PagedResponse<Tag>> {
        let path = format!("{}/items/{}/tags", self.user_prefix(), key);
        self.get_json_array(&path, params.to_query_pairs()).await
    }

    /// List all tags used across all items.
    ///
    /// `GET /users/<id>/items/tags`
    pub async fn list_items_tags(&self, params: &TagListParams) -> Result<PagedResponse<Tag>> {
        let path = format!("{}/items/tags", self.user_prefix());
        self.get_json_array(&path, params.to_query_pairs()).await
    }

    /// List tags used across top-level items.
    ///
    /// `GET /users/<id>/items/top/tags`
    pub async fn list_top_items_tags(&self, params: &TagListParams) -> Result<PagedResponse<Tag>> {
        let path = format!("{}/items/top/tags", self.user_prefix());
        self.get_json_array(&path, params.to_query_pairs()).await
    }

    /// List tags used across trashed items.
    ///
    /// `GET /users/<id>/items/trash/tags`
    pub async fn list_trash_tags(&self, params: &TagListParams) -> Result<PagedResponse<Tag>> {
        let path = format!("{}/items/trash/tags", self.user_prefix());
        self.get_json_array(&path, params.to_query_pairs()).await
    }

    /// List tags used across items in a collection.
    ///
    /// `GET /users/<id>/collections/<key>/tags`
    pub async fn list_collection_tags(
        &self,
        collection_key: &str,
        params: &TagListParams,
    ) -> Result<PagedResponse<Tag>> {
        let path = format!(
            "{}/collections/{}/tags",
            self.user_prefix(),
            collection_key
        );
        self.get_json_array(&path, params.to_query_pairs()).await
    }

    /// List tags across items in a collection.
    ///
    /// `GET /users/<id>/collections/<key>/items/tags`
    pub async fn list_collection_items_tags(
        &self,
        collection_key: &str,
        params: &TagListParams,
    ) -> Result<PagedResponse<Tag>> {
        let path = format!(
            "{}/collections/{}/items/tags",
            self.user_prefix(),
            collection_key
        );
        self.get_json_array(&path, params.to_query_pairs()).await
    }

    /// List tags across top-level items in a collection.
    ///
    /// `GET /users/<id>/collections/<key>/items/top/tags`
    pub async fn list_collection_top_items_tags(
        &self,
        collection_key: &str,
        params: &TagListParams,
    ) -> Result<PagedResponse<Tag>> {
        let path = format!(
            "{}/collections/{}/items/top/tags",
            self.user_prefix(),
            collection_key
        );
        self.get_json_array(&path, params.to_query_pairs()).await
    }

    /// List tags across publication items.
    ///
    /// `GET /users/<id>/publications/items/tags`
    ///
    /// **Quirk:** This endpoint returns ALL library tags, not just publication
    /// tags.
    pub async fn list_publication_tags(
        &self,
        params: &TagListParams,
    ) -> Result<PagedResponse<Tag>> {
        let path = format!("{}/publications/items/tags", self.user_prefix());
        self.get_json_array(&path, params.to_query_pairs()).await
    }

    // ── Group endpoints ────────────────────────────────────────────────

    /// List groups the user belongs to.
    ///
    /// `GET /users/<id>/groups`
    pub async fn list_groups(&self) -> Result<PagedResponse<Group>> {
        let path = format!("{}/groups", self.user_prefix());
        self.get_json_array(&path, vec![]).await
    }

    // ── Key info endpoint ──────────────────────────────────────────────

    /// Get information about the current API key.
    ///
    /// `GET /keys/<key>`
    pub async fn get_key_info(&self) -> Result<serde_json::Value> {
        let path = format!("/keys/{}", self.api_key);
        self.get_json_single(&path, vec![]).await
    }
}

/// Minimal percent-encoding for tag names in URL paths.
fn urlencoded(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~' => out.push(byte as char),
            b' ' => out.push_str("%20"),
            _ => {
                out.push('%');
                out.push(char::from(b"0123456789ABCDEF"[(byte >> 4) as usize]));
                out.push(char::from(b"0123456789ABCDEF"[(byte & 0xf) as usize]));
            }
        }
    }
    out
}

/// Internal type for caching array responses with header metadata.
#[derive(serde::Serialize, serde::Deserialize)]
struct CachedArrayResponse {
    body: String,
    total_results: Option<u64>,
    last_modified_version: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::DiskCache;
    use std::time::Duration;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn item_list_json() -> String {
        r#"[{
            "key": "ABC12345",
            "version": 100,
            "library": { "type": "user", "id": 1, "name": "test", "links": {} },
            "links": {},
            "meta": {},
            "data": {
                "key": "ABC12345",
                "version": 100,
                "itemType": "journalArticle",
                "title": "Test",
                "creators": [],
                "tags": [],
                "collections": [],
                "relations": {},
                "dateAdded": "2024-01-01T00:00:00Z",
                "dateModified": "2024-01-01T00:00:00Z"
            }
        }]"#
        .to_string()
    }

    fn single_item_json() -> String {
        r#"{
            "key": "ABC12345",
            "version": 100,
            "library": { "type": "user", "id": 1, "name": "test", "links": {} },
            "links": {},
            "meta": {},
            "data": {
                "key": "ABC12345",
                "version": 100,
                "itemType": "journalArticle",
                "title": "Test",
                "creators": [],
                "tags": [],
                "collections": [],
                "relations": {},
                "dateAdded": "2024-01-01T00:00:00Z",
                "dateModified": "2024-01-01T00:00:00Z"
            }
        }"#
        .to_string()
    }

    fn collection_list_json() -> String {
        r#"[{
            "key": "COL12345",
            "version": 50,
            "library": { "type": "user", "id": 1, "name": "test", "links": {} },
            "links": {},
            "meta": { "numCollections": 0, "numItems": 5 },
            "data": {
                "key": "COL12345",
                "version": 50,
                "name": "Test Collection",
                "parentCollection": false,
                "relations": {}
            }
        }]"#
        .to_string()
    }

    fn tag_list_json() -> String {
        r#"[{
            "tag": "TestTag",
            "links": {},
            "meta": { "type": 0, "numItems": 3 }
        }]"#
        .to_string()
    }

    async fn setup_client(server: &MockServer) -> ZoteroClient {
        ZoteroClient::new("12345", "test-key").with_base_url(server.uri())
    }

    fn array_response(body: &str) -> ResponseTemplate {
        ResponseTemplate::new(200)
            .set_body_string(body.to_string())
            .insert_header("Total-Results", "42")
            .insert_header("Last-Modified-Version", "100")
    }

    // ── Item list tests ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_items() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items"))
            .and(header("Zotero-API-Version", "3"))
            .and(header("Zotero-API-Key", "test-key"))
            .respond_with(array_response(&item_list_json()))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client.list_items(&ItemListParams::default()).await.unwrap();
        assert_eq!(resp.items.len(), 1);
        assert_eq!(resp.total_results, Some(42));
        assert_eq!(resp.last_modified_version, Some(100));
        assert_eq!(resp.items[0].key, "ABC12345");
    }

    #[tokio::test]
    async fn test_list_top_items() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items/top"))
            .respond_with(array_response(&item_list_json()))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client
            .list_top_items(&ItemListParams::default())
            .await
            .unwrap();
        assert_eq!(resp.items.len(), 1);
    }

    #[tokio::test]
    async fn test_list_trash_items() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items/trash"))
            .respond_with(array_response(&item_list_json()))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client
            .list_trash_items(&ItemListParams::default())
            .await
            .unwrap();
        assert_eq!(resp.items.len(), 1);
    }

    #[tokio::test]
    async fn test_get_item() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items/ABC12345"))
            .respond_with(ResponseTemplate::new(200).set_body_string(single_item_json()))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let item = client.get_item("ABC12345").await.unwrap();
        assert_eq!(item.key, "ABC12345");
    }

    #[tokio::test]
    async fn test_list_item_children() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items/ABC12345/children"))
            .respond_with(array_response(&item_list_json()))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client
            .list_item_children("ABC12345", &ItemListParams::default())
            .await
            .unwrap();
        assert_eq!(resp.items.len(), 1);
    }

    #[tokio::test]
    async fn test_list_collection_items() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/collections/COL1/items"))
            .respond_with(array_response(&item_list_json()))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client
            .list_collection_items("COL1", &ItemListParams::default())
            .await
            .unwrap();
        assert_eq!(resp.items.len(), 1);
    }

    #[tokio::test]
    async fn test_list_collection_top_items() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/collections/COL1/items/top"))
            .respond_with(array_response(&item_list_json()))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client
            .list_collection_top_items("COL1", &ItemListParams::default())
            .await
            .unwrap();
        assert_eq!(resp.items.len(), 1);
    }

    // ── Item params test ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_item_list_with_params() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items"))
            .and(query_param("q", "test"))
            .and(query_param("itemType", "book"))
            .and(query_param("sort", "dateModified"))
            .and(query_param("direction", "desc"))
            .and(query_param("limit", "5"))
            .respond_with(array_response(&item_list_json()))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let params = ItemListParams::builder()
            .q("test")
            .item_type("book")
            .sort("dateModified")
            .direction("desc")
            .limit(5)
            .build();
        let resp = client.list_items(&params).await.unwrap();
        assert_eq!(resp.items.len(), 1);
    }

    // ── Collection tests ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_collections() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/collections"))
            .respond_with(array_response(&collection_list_json()))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client
            .list_collections(&CollectionListParams::default())
            .await
            .unwrap();
        assert_eq!(resp.items.len(), 1);
        assert_eq!(resp.items[0].data.name, "Test Collection");
    }

    #[tokio::test]
    async fn test_list_top_collections() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/collections/top"))
            .respond_with(array_response(&collection_list_json()))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client
            .list_top_collections(&CollectionListParams::default())
            .await
            .unwrap();
        assert_eq!(resp.items.len(), 1);
    }

    #[tokio::test]
    async fn test_get_collection() {
        let server = MockServer::start().await;
        let single_json = r#"{
            "key": "COL12345",
            "version": 50,
            "library": { "type": "user", "id": 1, "name": "test", "links": {} },
            "links": {},
            "meta": {},
            "data": { "key": "COL12345", "version": 50, "name": "Test", "parentCollection": false, "relations": {} }
        }"#;
        Mock::given(method("GET"))
            .and(path("/users/12345/collections/COL12345"))
            .respond_with(ResponseTemplate::new(200).set_body_string(single_json))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let coll = client.get_collection("COL12345").await.unwrap();
        assert_eq!(coll.key, "COL12345");
    }

    #[tokio::test]
    async fn test_list_subcollections() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/collections/COL1/collections"))
            .respond_with(array_response(&collection_list_json()))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client
            .list_subcollections("COL1", &CollectionListParams::default())
            .await
            .unwrap();
        assert_eq!(resp.items.len(), 1);
    }

    // ── Tag tests ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_tags() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/tags"))
            .respond_with(array_response(&tag_list_json()))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client.list_tags(&TagListParams::default()).await.unwrap();
        assert_eq!(resp.items.len(), 1);
        assert_eq!(resp.items[0].tag, "TestTag");
    }

    #[tokio::test]
    async fn test_list_items_tags() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items/tags"))
            .respond_with(array_response(&tag_list_json()))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client
            .list_items_tags(&TagListParams::default())
            .await
            .unwrap();
        assert_eq!(resp.items.len(), 1);
    }

    #[tokio::test]
    async fn test_list_collection_tags() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/collections/COL1/tags"))
            .respond_with(array_response(&tag_list_json()))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client
            .list_collection_tags("COL1", &TagListParams::default())
            .await
            .unwrap();
        assert_eq!(resp.items.len(), 1);
    }

    // ── Search tests ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_searches() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/searches"))
            .respond_with(array_response("[]"))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client.list_searches().await.unwrap();
        assert!(resp.items.is_empty());
    }

    // ── Group tests ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_groups() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/groups"))
            .respond_with(array_response("[]"))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client.list_groups().await.unwrap();
        assert!(resp.items.is_empty());
    }

    // ── Error tests ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_error_404() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items/NOTFOUND"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not found"))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let err = client.get_item("NOTFOUND").await.unwrap_err();
        match err {
            ZoteroError::Api { status, message } => {
                assert_eq!(status, 404);
                assert_eq!(message, "Not found");
            }
            _ => panic!("Expected Api error, got {:?}", err),
        }
    }

    #[tokio::test]
    async fn test_error_403() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items"))
            .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let err = client
            .list_items(&ItemListParams::default())
            .await
            .unwrap_err();
        match err {
            ZoteroError::Api { status, .. } => assert_eq!(status, 403),
            _ => panic!("Expected Api error"),
        }
    }

    // ── Header extraction test ────────────────────────────────────────

    #[tokio::test]
    async fn test_header_extraction() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("[]")
                    .insert_header("Total-Results", "999")
                    .insert_header("Last-Modified-Version", "42"),
            )
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client.list_items(&ItemListParams::default()).await.unwrap();
        assert_eq!(resp.total_results, Some(999));
        assert_eq!(resp.last_modified_version, Some(42));
    }

    #[tokio::test]
    async fn test_missing_headers() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items"))
            .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client.list_items(&ItemListParams::default()).await.unwrap();
        assert_eq!(resp.total_results, None);
        assert_eq!(resp.last_modified_version, None);
    }

    // ── Cache tests ───────────────────────────────────────────────────

    fn temp_cache() -> DiskCache {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use std::time::{SystemTime, UNIX_EPOCH};
        let mut h = DefaultHasher::new();
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut h);
        std::thread::current().id().hash(&mut h);
        let dir = std::env::temp_dir()
            .join("papers-zotero-test-cache")
            .join(format!("{:x}", h.finish()));
        DiskCache::new(dir, Duration::from_secs(600)).unwrap()
    }

    #[tokio::test]
    async fn test_cache_hit_avoids_second_request() {
        let server = MockServer::start().await;
        let mock = Mock::given(method("GET"))
            .and(path("/users/12345/items"))
            .respond_with(array_response(&item_list_json()))
            .expect(1)
            .named("list_items")
            .mount_as_scoped(&server)
            .await;
        let client = ZoteroClient::new("12345", "test-key")
            .with_base_url(server.uri())
            .with_cache(temp_cache());
        let resp1 = client.list_items(&ItemListParams::default()).await.unwrap();
        assert_eq!(resp1.items.len(), 1);
        // Second call from cache
        let resp2 = client.list_items(&ItemListParams::default()).await.unwrap();
        assert_eq!(resp2.items.len(), 1);
        assert_eq!(resp2.total_results, Some(42));
        drop(mock);
    }

    #[tokio::test]
    async fn test_cache_error_not_cached() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items/bad"))
            .respond_with(ResponseTemplate::new(500).set_body_string("error"))
            .expect(2)
            .mount(&server)
            .await;
        let client = ZoteroClient::new("12345", "test-key")
            .with_base_url(server.uri())
            .with_cache(temp_cache());
        let _ = client.get_item("bad").await;
        let _ = client.get_item("bad").await;
    }

    // ── File download test ────────────────────────────────────────────

    #[tokio::test]
    async fn test_download_item_file() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items/ATTACH1/file"))
            .and(header("Zotero-API-Version", "3"))
            .and(header("Zotero-API-Key", "test-key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(b"fake-pdf-bytes".to_vec()),
            )
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let bytes = client.download_item_file("ATTACH1").await.unwrap();
        assert_eq!(bytes, b"fake-pdf-bytes");
    }

    #[tokio::test]
    async fn test_download_item_file_404() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items/MISSING/file"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not found"))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let err = client.download_item_file("MISSING").await.unwrap_err();
        match err {
            ZoteroError::Api { status, .. } => assert_eq!(status, 404),
            _ => panic!("Expected Api error, got {:?}", err),
        }
    }

    // ── URL encoding test ─────────────────────────────────────────────

    #[test]
    fn test_urlencoded() {
        assert_eq!(urlencoded("simple"), "simple");
        assert_eq!(urlencoded("with space"), "with%20space");
        assert_eq!(urlencoded("special/chars&more"), "special%2Fchars%26more");
    }
}
