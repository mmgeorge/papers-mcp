use crate::cache::DiskCache;
use crate::error::{Result, ZoteroError};
use crate::params::{CollectionListParams, DeletedParams, FulltextParams, ItemListParams, TagListParams};
use crate::response::{PagedResponse, VersionedResponse};
use crate::types::*;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

const DEFAULT_BASE_URL: &str = "https://api.zotero.org";

/// Returns the path to the Zotero executable if it is found on disk, or
/// `None` if Zotero does not appear to be installed.
fn find_zotero_exe() -> Option<String> {
    let mut candidates: Vec<String> = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Ok(pf) = std::env::var("PROGRAMFILES") {
            candidates.push(format!(r"{pf}\Zotero\zotero.exe"));
        } else {
            candidates.push(r"C:\Program Files\Zotero\zotero.exe".into());
        }
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            candidates.push(format!(r"{local}\Zotero\zotero.exe"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        candidates.push("/Applications/Zotero.app/Contents/MacOS/zotero".into());
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = std::env::var("HOME") {
            candidates.push(format!("{home}/Zotero/zotero"));
        }
        candidates.push("/opt/Zotero/zotero".into());
        candidates.push("/usr/lib/zotero/zotero".into());
        candidates.push("/usr/bin/zotero".into());
    }

    candidates.into_iter().find(|p| std::path::Path::new(p).exists())
}

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

    /// Create a client from environment variables, preferring the local Zotero
    /// API (`http://localhost:23119`) if it is running and has the local API
    /// enabled. Falls back to the web API with disk cache when unavailable.
    ///
    /// Use this instead of `from_env()` for interactive tools where low latency
    /// matters. The local API requires "Enable Local API" to be turned on in
    /// Zotero → Settings → Advanced.
    pub async fn from_env_prefer_local() -> Result<Self> {
        let user_id = std::env::var("ZOTERO_USER_ID").map_err(|_| ZoteroError::Api {
            status: 0,
            message: "ZOTERO_USER_ID environment variable not set".into(),
        })?;
        let api_key = std::env::var("ZOTERO_API_KEY").map_err(|_| ZoteroError::Api {
            status: 0,
            message: "ZOTERO_API_KEY environment variable not set".into(),
        })?;

        const LOCAL_BASE: &str = "http://127.0.0.1:23119/api";
        let probe_url = format!("{LOCAL_BASE}/users/{user_id}/items?limit=0");
        let local_ok = reqwest::Client::new()
            .get(&probe_url)
            .timeout(std::time::Duration::from_millis(500))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);

        if local_ok {
            // Local API is up — no cache needed, it's all in-process on this machine.
            Ok(Self::new(user_id, api_key).with_base_url(LOCAL_BASE))
        } else {
            // If Zotero is installed but not running, surface an actionable error
            // rather than silently falling back to the slower remote API.
            // Set ZOTERO_CHECK_LAUNCHED=0 to opt out of this check.
            let skip_check = std::env::var("ZOTERO_CHECK_LAUNCHED")
                .map(|v| v == "0")
                .unwrap_or(false);
            if !skip_check {
                if let Some(path) = find_zotero_exe() {
                    return Err(ZoteroError::NotRunning { path });
                }
            }
            // Zotero not found on disk — fall back to the web API.
            // Cache disabled: write operations (upload/download) would make
            // subsequent list calls return stale results within the TTL window.
            // Re-enable once cache invalidation on write is implemented.
            // let mut client = Self::new(&user_id, &api_key);
            // if let Ok(c) = DiskCache::default_location(std::time::Duration::from_secs(60)) {
            //     client = client.with_cache(c);
            // }
            Ok(Self::new(user_id, api_key))
        }
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

    /// GET request returning a single JSON object plus `Last-Modified-Version`.
    ///
    /// Used by endpoints that return one object (not an array) but still carry
    /// the `Last-Modified-Version` sync header (fulltext, deleted, settings).
    async fn get_json_versioned<T: DeserializeOwned>(
        &self,
        path: &str,
        query: Vec<(&str, String)>,
    ) -> Result<VersionedResponse<T>> {
        let url = format!("{}{}", self.base_url, path);
        if let Some(cache) = &self.cache
            && let Some(text) = cache.get(&url, &query, None)
        {
            let cached: CachedVersionedResponse =
                serde_json::from_str(&text).map_err(ZoteroError::Json)?;
            let data: T = serde_json::from_str(&cached.body).map_err(ZoteroError::Json)?;
            return Ok(VersionedResponse {
                data,
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
        let last_modified_version = resp
            .headers()
            .get("Last-Modified-Version")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok());
        let text = resp.text().await?;
        if let Some(cache) = &self.cache {
            let cached = CachedVersionedResponse {
                body: text.clone(),
                last_modified_version,
            };
            if let Ok(cache_text) = serde_json::to_string(&cached) {
                cache.set(&url, &query, None, &cache_text);
            }
        }
        let data: T = serde_json::from_str(&text).map_err(ZoteroError::Json)?;
        Ok(VersionedResponse {
            data,
            last_modified_version,
        })
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

    /// Create a child attachment item under a parent.
    ///
    /// `POST /users/<id>/items`
    ///
    /// Returns the new attachment item's key.
    pub async fn create_imported_attachment(
        &self,
        parent_key: &str,
        filename: &str,
        content_type: &str,
    ) -> Result<String> {
        let item = serde_json::json!([{
            "itemType": "attachment",
            "parentItem": parent_key,
            "linkMode": "imported_file",
            "title": filename,
            "filename": filename,
            "contentType": content_type,
            "tags": [],
            "collections": []
        }]);
        let path = format!("{}/items", self.user_prefix());
        let resp = self.post_json_write(&path, &item).await?;
        resp.successful
            .get("0")
            .and_then(|v| v.get("key"))
            .and_then(|k| k.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ZoteroError::Api {
                status: 0,
                message: "create_imported_attachment: no key in successful[\"0\"]".into(),
            })
    }

    /// Upload file bytes to an attachment item using Zotero's 3-step S3 protocol.
    ///
    /// `POST /users/<id>/items/<key>/file`
    pub async fn upload_attachment_file(
        &self,
        attachment_key: &str,
        filename: &str,
        data: Vec<u8>,
    ) -> Result<()> {
        use md5::{Digest, Md5};

        // Step 1: compute md5, size, mtime
        let hash = Md5::digest(&data);
        let md5_hex = format!("{:x}", hash);
        let filesize = data.len();
        let mtime = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);

        // Step 2: register upload
        let path = format!("{}/items/{}/file", self.user_prefix(), attachment_key);
        let url = format!("{}{}", self.base_url, path);
        let register_body = format!(
            "md5={}&filename={}&filesize={}&mtime={}",
            md5_hex, filename, filesize, mtime
        );
        let resp = self
            .http
            .post(&url)
            .header("Zotero-API-Version", "3")
            .header("Zotero-API-Key", &self.api_key)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("If-None-Match", "*")
            .body(register_body)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(ZoteroError::Api { status: status.as_u16(), message });
        }
        let register_text = resp.text().await?;
        let register_json: serde_json::Value =
            serde_json::from_str(&register_text).map_err(ZoteroError::Json)?;

        // If file already exists on S3, we're done
        if register_json.get("exists").and_then(|v| v.as_u64()) == Some(1) {
            return Ok(());
        }

        // Step 3: upload to S3
        let s3_url = register_json["url"]
            .as_str()
            .ok_or_else(|| ZoteroError::Api { status: 0, message: "upload response missing url".into() })?
            .to_string();
        let s3_content_type = register_json["contentType"]
            .as_str()
            .ok_or_else(|| ZoteroError::Api { status: 0, message: "upload response missing contentType".into() })?
            .to_string();
        let prefix = register_json["prefix"].as_str().unwrap_or("").as_bytes().to_vec();
        let suffix = register_json["suffix"].as_str().unwrap_or("").as_bytes().to_vec();
        let upload_key = register_json["uploadKey"]
            .as_str()
            .ok_or_else(|| ZoteroError::Api { status: 0, message: "upload response missing uploadKey".into() })?
            .to_string();

        let mut body = prefix;
        body.extend_from_slice(&data);
        body.extend_from_slice(&suffix);

        let s3_resp = self
            .http
            .post(&s3_url)
            .header("Content-Type", s3_content_type)
            .body(body)
            .send()
            .await?;
        let s3_status = s3_resp.status();
        if !s3_status.is_success() {
            let message = s3_resp.text().await.unwrap_or_default();
            return Err(ZoteroError::Api { status: s3_status.as_u16(), message });
        }

        // Step 4: register completion
        let complete_body = format!("upload={}", upload_key);
        let complete_resp = self
            .http
            .post(&url)
            .header("Zotero-API-Version", "3")
            .header("Zotero-API-Key", &self.api_key)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("If-None-Match", "*")
            .body(complete_body)
            .send()
            .await?;
        let complete_status = complete_resp.status();
        if !complete_status.is_success() {
            let message = complete_resp.text().await.unwrap_or_default();
            return Err(ZoteroError::Api { status: complete_status.as_u16(), message });
        }

        Ok(())
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

    // ── Full-text endpoints ────────────────────────────────────────────

    /// List all attachment items that have indexed full-text content, mapped
    /// to their current full-text version number.
    ///
    /// `GET /users/<id>/fulltext`
    ///
    /// Returns a `HashMap<item_key, version>`. Use the `since` param to
    /// fetch only items whose full-text changed after a given library version.
    /// The `last_modified_version` in the response is the current library
    /// version — use it as your next `since` checkpoint.
    pub async fn list_fulltext_versions(
        &self,
        params: &FulltextParams,
    ) -> Result<VersionedResponse<HashMap<String, u64>>> {
        let path = format!("{}/fulltext", self.user_prefix());
        self.get_json_versioned(&path, params.to_query_pairs()).await
    }

    /// Get the full-text content of a single attachment item.
    ///
    /// `GET /users/<id>/items/<key>/fulltext`
    ///
    /// Returns 404 if the item has no indexed content.
    /// PDF attachments populate `indexed_pages` / `total_pages`;
    /// other document types populate `indexed_chars` / `total_chars`.
    ///
    /// When the local Zotero connector is active and the API endpoint is not
    /// available (e.g. older Zotero 7.0.x builds), automatically falls back to
    /// reading the `.zotero-ft-cache` file from local Zotero storage.
    pub async fn get_item_fulltext(&self, key: &str) -> Result<VersionedResponse<ItemFulltext>> {
        let path = format!("{}/items/{}/fulltext", self.user_prefix(), key);
        match self.get_json_versioned(&path, vec![]).await {
            Ok(r) => Ok(r),
            Err(ZoteroError::Api { status: 404, .. }) => {
                // The local Zotero connector may not expose /fulltext (older
                // builds).  Fall back to reading the .zotero-ft-cache file
                // that Zotero writes alongside every indexed PDF.
                self.get_item_fulltext_from_cache(key).await
            }
            Err(e) => Err(e),
        }
    }

    /// Read fulltext from Zotero's local `.zotero-ft-cache` file.
    ///
    /// Called automatically by [`get_item_fulltext`] when the API returns 404.
    async fn get_item_fulltext_from_cache(
        &self,
        key: &str,
    ) -> Result<VersionedResponse<ItemFulltext>> {
        let file_url_str = self.get_item_file_view_url(key).await?;
        if let Ok(file_url) = reqwest::Url::parse(&file_url_str) {
            if file_url.scheme() == "file" {
                if let Ok(pdf_path) = file_url.to_file_path() {
                    let cache_path = pdf_path
                        .parent()
                        .unwrap_or(pdf_path.as_path())
                        .join(".zotero-ft-cache");
                    if cache_path.exists() {
                        let content =
                            std::fs::read_to_string(&cache_path).map_err(|e| {
                                ZoteroError::Api {
                                    status: 0,
                                    message: format!(
                                        "local ft-cache read error ({}): {e}",
                                        cache_path.display()
                                    ),
                                }
                            })?;
                        return Ok(VersionedResponse {
                            data: ItemFulltext {
                                content,
                                indexed_pages: None,
                                total_pages: None,
                                indexed_chars: None,
                                total_chars: None,
                            },
                            last_modified_version: None,
                        });
                    }
                }
            }
        }
        Err(ZoteroError::Api {
            status: 404,
            message: "Fulltext not indexed or cache file not found".to_string(),
        })
    }

    // ── Deleted-objects endpoint ───────────────────────────────────────

    /// Get all objects deleted from the library since a given version.
    ///
    /// `GET /users/<id>/deleted?since=<version>`
    ///
    /// The `since` parameter is required; pass `0` to get the full deletion
    /// history. Use `last_modified_version` from the response as your next
    /// `since` checkpoint for incremental sync.
    pub async fn get_deleted(
        &self,
        params: &DeletedParams,
    ) -> Result<VersionedResponse<DeletedObjects>> {
        let path = format!("{}/deleted", self.user_prefix());
        self.get_json_versioned(&path, params.to_query_pairs()).await
    }

    // ── Settings endpoints ─────────────────────────────────────────────

    /// Get all user settings as a map of setting key → [`SettingEntry`].
    ///
    /// `GET /users/<id>/settings`
    ///
    /// Known keys include `tagColors` (an array of `{name, color}` objects)
    /// and `lastPageIndex_u_<itemKey>` (last-viewed page for a PDF).
    pub async fn get_settings(
        &self,
    ) -> Result<VersionedResponse<HashMap<String, SettingEntry>>> {
        let path = format!("{}/settings", self.user_prefix());
        self.get_json_versioned(&path, vec![]).await
    }

    /// Get a single user setting by key.
    ///
    /// `GET /users/<id>/settings/<key>`
    ///
    /// Returns 404 if the setting key does not exist.
    pub async fn get_setting(&self, key: &str) -> Result<VersionedResponse<SettingEntry>> {
        let path = format!("{}/settings/{}", self.user_prefix(), key);
        self.get_json_versioned(&path, vec![]).await
    }

    // ── File view endpoints ────────────────────────────────────────────

    /// Download the file content of an attachment via the browser-view URL.
    ///
    /// `GET /users/<id>/items/<key>/file/view`
    ///
    /// On the remote Zotero API this follows a 302 redirect to the stable CDN
    /// URL and returns the raw bytes.  The local Zotero connector instead
    /// returns a `302 Location: file:///…` pointing at the local Zotero
    /// storage directory; in that case the file is read directly from disk.
    ///
    /// [`download_item_file`]: ZoteroClient::download_item_file
    pub async fn get_item_file_view(&self, key: &str) -> Result<Vec<u8>> {
        let path = format!("{}/items/{}/file/view", self.user_prefix(), key);
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .get(&url)
            .header("Zotero-API-Version", "3")
            .header("Zotero-API-Key", &self.api_key)
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() {
            return Ok(resp.bytes().await?.to_vec());
        }
        // The local Zotero connector redirects to file:// which reqwest cannot
        // follow.  Detect that case and read the file from disk instead.
        if status.as_u16() == 302 {
            if let Some(location) = resp.headers().get("Location") {
                if let Ok(loc_str) = location.to_str() {
                    if let Ok(file_url) = reqwest::Url::parse(loc_str) {
                        if file_url.scheme() == "file" {
                            if let Ok(file_path) = file_url.to_file_path() {
                                return std::fs::read(&file_path).map_err(|e| {
                                    ZoteroError::Api {
                                        status: 0,
                                        message: format!(
                                            "local file read error ({}): {e}",
                                            file_path.display()
                                        ),
                                    }
                                });
                            }
                        }
                    }
                }
            }
        }
        let message = resp.text().await.unwrap_or_default();
        Err(ZoteroError::Api { status: status.as_u16(), message })
    }

    /// Get a pre-signed CDN URL for viewing an attachment file.
    ///
    /// `GET /users/<id>/items/<key>/file/view/url`
    ///
    /// Returns the redirect target URL as a plain string without following the
    /// redirect. Useful when you need the URL itself rather than the content —
    /// e.g. to pass to a browser or PDF viewer.
    pub async fn get_item_file_view_url(&self, key: &str) -> Result<String> {
        let path = format!("{}/items/{}/file/view/url", self.user_prefix(), key);
        let bytes = self.get_binary(&path).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    // ── Write helpers ──────────────────────────────────────────────────

    /// POST a JSON body, expecting a `200 OK` with a [`WriteResponse`] body.
    async fn post_json_write(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<WriteResponse> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .post(&url)
            .header("Zotero-API-Version", "3")
            .header("Zotero-API-Key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(ZoteroError::Api { status: status.as_u16(), message });
        }
        let text = resp.text().await?;
        serde_json::from_str(&text).map_err(ZoteroError::Json)
    }

    /// PUT a JSON body to a single-object path, expecting `204 No Content`.
    /// Requires `If-Unmodified-Since-Version` for optimistic concurrency.
    async fn put_no_content(
        &self,
        path: &str,
        version: u64,
        body: &serde_json::Value,
    ) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .put(&url)
            .header("Zotero-API-Version", "3")
            .header("Zotero-API-Key", &self.api_key)
            .header("Content-Type", "application/json")
            .header("If-Unmodified-Since-Version", version.to_string())
            .json(body)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(ZoteroError::Api { status: status.as_u16(), message });
        }
        Ok(())
    }

    /// PATCH a JSON body to a single-object path, expecting `204 No Content`.
    /// Requires `If-Unmodified-Since-Version` for optimistic concurrency.
    async fn patch_no_content(
        &self,
        path: &str,
        version: u64,
        body: &serde_json::Value,
    ) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .patch(&url)
            .header("Zotero-API-Version", "3")
            .header("Zotero-API-Key", &self.api_key)
            .header("Content-Type", "application/json")
            .header("If-Unmodified-Since-Version", version.to_string())
            .json(body)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(ZoteroError::Api { status: status.as_u16(), message });
        }
        Ok(())
    }

    /// DELETE a single resource by path, expecting `204 No Content`.
    /// Requires `If-Unmodified-Since-Version` for optimistic concurrency.
    async fn delete_no_content(&self, path: &str, version: u64) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .delete(&url)
            .header("Zotero-API-Version", "3")
            .header("Zotero-API-Key", &self.api_key)
            .header("If-Unmodified-Since-Version", version.to_string())
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(ZoteroError::Api { status: status.as_u16(), message });
        }
        Ok(())
    }

    /// DELETE multiple resources by appending a query parameter, expecting
    /// `204 No Content`. The `library_version` goes in
    /// `If-Unmodified-Since-Version`.
    async fn delete_multiple_no_content(
        &self,
        path: &str,
        query_key: &str,
        values: &[String],
        library_version: u64,
    ) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let joined = values.join(",");
        let resp = self
            .http
            .delete(&url)
            .header("Zotero-API-Version", "3")
            .header("Zotero-API-Key", &self.api_key)
            .header("If-Unmodified-Since-Version", library_version.to_string())
            .query(&[(query_key, &joined)])
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(ZoteroError::Api { status: status.as_u16(), message });
        }
        Ok(())
    }

    // ── Item write endpoints ───────────────────────────────────────────

    /// Create one or more items.
    ///
    /// `POST /users/<id>/items`
    ///
    /// Each element of `items` must be a JSON object containing at least
    /// `itemType`. Returns a [`WriteResponse`] with `successful`, `unchanged`,
    /// and `failed` maps keyed by the input array index.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # async fn example() -> papers_zotero::Result<()> {
    /// use papers_zotero::ZoteroClient;
    /// use serde_json::json;
    ///
    /// let client = ZoteroClient::from_env()?;
    /// let result = client.create_items(vec![
    ///     json!({ "itemType": "note", "note": "Hello from Rust" })
    /// ]).await?;
    /// println!("created keys: {:?}", result.successful_keys());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_items(&self, items: Vec<serde_json::Value>) -> Result<WriteResponse> {
        let path = format!("{}/items", self.user_prefix());
        self.post_json_write(&path, &serde_json::Value::Array(items)).await
    }

    /// Fully replace a single item.
    ///
    /// `PUT /users/<id>/items/<key>`
    ///
    /// `version` must match the item's current version; pass the value from
    /// a prior `get_item` call. Returns `412 Precondition Failed` if the item
    /// has been modified since.
    pub async fn update_item(
        &self,
        key: &str,
        version: u64,
        data: serde_json::Value,
    ) -> Result<()> {
        let path = format!("{}/items/{}", self.user_prefix(), key);
        self.put_no_content(&path, version, &data).await
    }

    /// Partially update a single item (only supply changed fields).
    ///
    /// `PATCH /users/<id>/items/<key>`
    ///
    /// `version` must match the item's current version.
    pub async fn patch_item(
        &self,
        key: &str,
        version: u64,
        data: serde_json::Value,
    ) -> Result<()> {
        let path = format!("{}/items/{}", self.user_prefix(), key);
        self.patch_no_content(&path, version, &data).await
    }

    /// Delete a single item.
    ///
    /// `DELETE /users/<id>/items/<key>`
    ///
    /// `version` must match the item's current version.
    pub async fn delete_item(&self, key: &str, version: u64) -> Result<()> {
        let path = format!("{}/items/{}", self.user_prefix(), key);
        self.delete_no_content(&path, version).await
    }

    /// Delete multiple items in a single request.
    ///
    /// `DELETE /users/<id>/items?itemKey=<key>,<key>,...`
    ///
    /// `library_version` must be the current library version (from a prior
    /// list or write response).
    pub async fn delete_items(&self, keys: &[String], library_version: u64) -> Result<()> {
        let path = format!("{}/items", self.user_prefix());
        self.delete_multiple_no_content(&path, "itemKey", keys, library_version).await
    }

    // ── Collection write endpoints ─────────────────────────────────────

    /// Create one or more collections.
    ///
    /// `POST /users/<id>/collections`
    ///
    /// Each element must contain at least `name`. Optionally include
    /// `parentCollection` (key string) for nested collections.
    pub async fn create_collections(
        &self,
        collections: Vec<serde_json::Value>,
    ) -> Result<WriteResponse> {
        let path = format!("{}/collections", self.user_prefix());
        self.post_json_write(&path, &serde_json::Value::Array(collections)).await
    }

    /// Fully replace a single collection.
    ///
    /// `PUT /users/<id>/collections/<key>`
    pub async fn update_collection(
        &self,
        key: &str,
        version: u64,
        data: serde_json::Value,
    ) -> Result<()> {
        let path = format!("{}/collections/{}", self.user_prefix(), key);
        self.put_no_content(&path, version, &data).await
    }

    /// Delete a single collection.
    ///
    /// `DELETE /users/<id>/collections/<key>`
    pub async fn delete_collection(&self, key: &str, version: u64) -> Result<()> {
        let path = format!("{}/collections/{}", self.user_prefix(), key);
        self.delete_no_content(&path, version).await
    }

    /// Delete multiple collections in a single request.
    ///
    /// `DELETE /users/<id>/collections?collectionKey=<key>,<key>,...`
    pub async fn delete_collections(
        &self,
        keys: &[String],
        library_version: u64,
    ) -> Result<()> {
        let path = format!("{}/collections", self.user_prefix());
        self.delete_multiple_no_content(&path, "collectionKey", keys, library_version).await
    }

    // ── Search write endpoints ─────────────────────────────────────────

    /// Create one or more saved searches.
    ///
    /// `POST /users/<id>/searches`
    ///
    /// Each element must contain `name` and `conditions` (array of
    /// `{condition, operator, value}` objects).
    pub async fn create_searches(
        &self,
        searches: Vec<serde_json::Value>,
    ) -> Result<WriteResponse> {
        let path = format!("{}/searches", self.user_prefix());
        self.post_json_write(&path, &serde_json::Value::Array(searches)).await
    }

    /// Delete multiple saved searches in a single request.
    ///
    /// `DELETE /users/<id>/searches?searchKey=<key>,<key>,...`
    pub async fn delete_searches(
        &self,
        keys: &[String],
        library_version: u64,
    ) -> Result<()> {
        let path = format!("{}/searches", self.user_prefix());
        self.delete_multiple_no_content(&path, "searchKey", keys, library_version).await
    }

    // ── Tag write endpoints ────────────────────────────────────────────

    /// Delete multiple tags from the library.
    ///
    /// `DELETE /users/<id>/tags?tag=<tag1> || <tag2> || ...`
    ///
    /// Tags are URL-encoded and joined with ` || `.
    pub async fn delete_tags(&self, tags: &[String], library_version: u64) -> Result<()> {
        let path = format!("{}/tags", self.user_prefix());
        let url = format!("{}{}", self.base_url, path);
        let tag_param = tags
            .iter()
            .map(|t| urlencoded(t))
            .collect::<Vec<_>>()
            .join(" || ");
        let resp = self
            .http
            .delete(&url)
            .header("Zotero-API-Version", "3")
            .header("Zotero-API-Key", &self.api_key)
            .header("If-Unmodified-Since-Version", library_version.to_string())
            .query(&[("tag", &tag_param)])
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(ZoteroError::Api { status: status.as_u16(), message });
        }
        Ok(())
    }

    // ── Key info endpoint ──────────────────────────────────────────────

    /// Get information about the current API key.
    ///
    /// `GET /keys/<key>`
    pub async fn get_key_info(&self) -> Result<serde_json::Value> {
        let path = format!("/keys/{}", self.api_key);
        self.get_json_single(&path, vec![]).await
    }

    /// Get information about the API key used for this request.
    ///
    /// `GET /keys/current`
    ///
    /// Equivalent to [`get_key_info`] but uses the `/keys/current` alias
    /// instead of embedding the key in the path.
    ///
    /// [`get_key_info`]: ZoteroClient::get_key_info
    pub async fn get_current_key_info(&self) -> Result<serde_json::Value> {
        self.get_json_single("/keys/current", vec![]).await
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

/// Internal type for caching versioned single-object responses.
#[derive(serde::Serialize, serde::Deserialize)]
struct CachedVersionedResponse {
    body: String,
    last_modified_version: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::DiskCache;
    use crate::params::{DeletedParams, FulltextParams};
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

    // ── Full-text tests ───────────────────────────────────────────────

    /// Real response shape (captured from live API, two entries):
    /// {"ZLIKNFNF":1385,"EYNDSWQJ":1399}
    /// Last-Modified-Version: 4384
    #[tokio::test]
    async fn test_list_fulltext_versions() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/fulltext"))
            .and(header("Zotero-API-Version", "3"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"ZLIKNFNF":1385,"EYNDSWQJ":1399}"#)
                    .insert_header("Last-Modified-Version", "4384"),
            )
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client
            .list_fulltext_versions(&FulltextParams::default())
            .await
            .unwrap();
        assert_eq!(resp.last_modified_version, Some(4384));
        assert_eq!(resp.data.len(), 2);
        assert_eq!(resp.data["ZLIKNFNF"], 1385);
        assert_eq!(resp.data["EYNDSWQJ"], 1399);
    }

    #[tokio::test]
    async fn test_list_fulltext_versions_since_param() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/fulltext"))
            .and(query_param("since", "1380"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"ZLIKNFNF":1385}"#)
                    .insert_header("Last-Modified-Version", "4384"),
            )
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let params = FulltextParams::builder().since(1380u64).build();
        let resp = client.list_fulltext_versions(&params).await.unwrap();
        assert_eq!(resp.data.len(), 1);
    }

    #[tokio::test]
    async fn test_list_fulltext_versions_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/fulltext"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("{}")
                    .insert_header("Last-Modified-Version", "4384"),
            )
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client
            .list_fulltext_versions(&FulltextParams::default())
            .await
            .unwrap();
        assert!(resp.data.is_empty());
        assert_eq!(resp.last_modified_version, Some(4384));
    }

    /// Real response shape (captured from live API, item 8HNHIZCE):
    /// {"content":"THEORY OF COMPUTING...","indexedPages":14,"totalPages":14}
    /// Last-Modified-Version: 13
    #[tokio::test]
    async fn test_get_item_fulltext() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items/ATTACH1/fulltext"))
            .and(header("Zotero-API-Version", "3"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(
                        r#"{"content":"Distance Transforms of Sampled Functions","indexedPages":14,"totalPages":14}"#,
                    )
                    .insert_header("Last-Modified-Version", "13"),
            )
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client.get_item_fulltext("ATTACH1").await.unwrap();
        assert_eq!(resp.last_modified_version, Some(13));
        assert_eq!(resp.data.indexed_pages, Some(14));
        assert_eq!(resp.data.total_pages, Some(14));
        assert!(resp.data.content.contains("Distance Transforms"));
    }

    #[tokio::test]
    async fn test_get_item_fulltext_not_indexed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items/NOTEXT/fulltext"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not found"))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let err = client.get_item_fulltext("NOTEXT").await.unwrap_err();
        match err {
            ZoteroError::Api { status, .. } => assert_eq!(status, 404),
            _ => panic!("Expected Api error"),
        }
    }

    #[tokio::test]
    async fn test_get_item_fulltext_char_indexed() {
        // Non-PDF documents report indexedChars/totalChars instead of pages
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items/HTML1/fulltext"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(
                        r#"{"content":"Some web content","indexedChars":500,"totalChars":1000}"#,
                    )
                    .insert_header("Last-Modified-Version", "77"),
            )
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client.get_item_fulltext("HTML1").await.unwrap();
        assert_eq!(resp.data.indexed_chars, Some(500));
        assert_eq!(resp.data.total_chars, Some(1000));
        assert!(resp.data.indexed_pages.is_none());
    }

    // ── Deleted tests ─────────────────────────────────────────────────

    /// Real response shape (captured from live API, condensed to 2 items):
    /// {"collections":["2WDMI6DR"],"items":["23IAQK5A","24F9PQTC"],
    ///  "searches":[],"tags":["old tag"],"settings":[]}
    /// Last-Modified-Version: 4384
    #[tokio::test]
    async fn test_get_deleted() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/deleted"))
            .and(query_param("since", "0"))
            .and(header("Zotero-API-Version", "3"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(
                        r#"{"collections":["2WDMI6DR"],"items":["23IAQK5A","24F9PQTC"],"searches":[],"tags":["old tag"],"settings":[]}"#,
                    )
                    .insert_header("Last-Modified-Version", "4384"),
            )
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let params = DeletedParams::builder().since(0u64).build();
        let resp = client.get_deleted(&params).await.unwrap();
        assert_eq!(resp.last_modified_version, Some(4384));
        assert_eq!(resp.data.collections, vec!["2WDMI6DR"]);
        assert_eq!(resp.data.items.len(), 2);
        assert_eq!(resp.data.tags, vec!["old tag"]);
        assert!(resp.data.searches.is_empty());
        assert!(resp.data.settings.is_empty());
    }

    #[tokio::test]
    async fn test_get_deleted_since_param_forwarded() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/deleted"))
            .and(query_param("since", "1000"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(
                        r#"{"collections":[],"items":[],"searches":[],"tags":[],"settings":[]}"#,
                    )
                    .insert_header("Last-Modified-Version", "4384"),
            )
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let params = DeletedParams::builder().since(1000u64).build();
        let resp = client.get_deleted(&params).await.unwrap();
        assert!(resp.data.items.is_empty());
    }

    #[tokio::test]
    async fn test_get_deleted_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/deleted"))
            .respond_with(ResponseTemplate::new(400).set_body_string("since parameter required"))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let params = DeletedParams::builder().since(0u64).build();
        let err = client.get_deleted(&params).await.unwrap_err();
        match err {
            ZoteroError::Api { status, .. } => assert_eq!(status, 400),
            _ => panic!("Expected Api error"),
        }
    }

    // ── Settings tests ────────────────────────────────────────────────

    /// Real response shape (captured from live API, condensed to one entry):
    /// {"tagColors":{"value":[{"name":"Starred","color":"#FF8C19"}],"version":3826}}
    /// Last-Modified-Version: 4384
    #[tokio::test]
    async fn test_get_settings() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/settings"))
            .and(header("Zotero-API-Version", "3"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(
                        r##"{"tagColors":{"value":[{"name":"Starred","color":"#FF8C19"}],"version":3826}}"##,
                    )
                    .insert_header("Last-Modified-Version", "4384"),
            )
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client.get_settings().await.unwrap();
        assert_eq!(resp.last_modified_version, Some(4384));
        let tag_colors = resp.data.get("tagColors").unwrap();
        assert_eq!(tag_colors.version, 3826);
        assert!(tag_colors.value.is_array());
    }

    #[tokio::test]
    async fn test_get_settings_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/settings"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("{}")
                    .insert_header("Last-Modified-Version", "100"),
            )
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client.get_settings().await.unwrap();
        assert!(resp.data.is_empty());
    }

    /// Real response shape (captured from live API, /settings/tagColors):
    /// {"value":[{"name":"Starred","color":"#FF8C19"},{"name":"Survey","color":"#FF6666"}],"version":3826}
    /// Last-Modified-Version: 3826
    #[tokio::test]
    async fn test_get_setting() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/settings/tagColors"))
            .and(header("Zotero-API-Version", "3"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(
                        r##"{"value":[{"name":"Starred","color":"#FF8C19"},{"name":"Survey","color":"#FF6666"}],"version":3826}"##,
                    )
                    .insert_header("Last-Modified-Version", "3826"),
            )
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client.get_setting("tagColors").await.unwrap();
        assert_eq!(resp.last_modified_version, Some(3826));
        assert_eq!(resp.data.version, 3826);
        let arr = resp.data.value.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["name"], "Starred");
    }

    #[tokio::test]
    async fn test_get_setting_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/settings/nonexistent"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not found"))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let err = client.get_setting("nonexistent").await.unwrap_err();
        match err {
            ZoteroError::Api { status, .. } => assert_eq!(status, 404),
            _ => panic!("Expected Api error"),
        }
    }

    // ── File view tests ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_item_file_view() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items/ATTACH1/file/view"))
            .and(header("Zotero-API-Version", "3"))
            .and(header("Zotero-API-Key", "test-key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(b"fake-pdf-bytes".to_vec()),
            )
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let bytes = client.get_item_file_view("ATTACH1").await.unwrap();
        assert_eq!(bytes, b"fake-pdf-bytes");
    }

    #[tokio::test]
    async fn test_get_item_file_view_404() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items/NOFILE/file/view"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not found"))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let err = client.get_item_file_view("NOFILE").await.unwrap_err();
        match err {
            ZoteroError::Api { status, .. } => assert_eq!(status, 404),
            _ => panic!("Expected Api error"),
        }
    }

    /// Real response shape (captured from live API, /items/8HNHIZCE/file/view/url):
    /// Content-Type: application/json, body is a plain URL string (no JSON quotes)
    #[tokio::test]
    async fn test_get_item_file_view_url() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items/ATTACH1/file/view/url"))
            .and(header("Zotero-API-Version", "3"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("https://files.zotero.net/abc123/paper.pdf"),
            )
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let url = client.get_item_file_view_url("ATTACH1").await.unwrap();
        assert_eq!(url, "https://files.zotero.net/abc123/paper.pdf");
    }

    #[tokio::test]
    async fn test_get_item_file_view_url_404() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/12345/items/NOFILE/file/view/url"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not found"))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let err = client.get_item_file_view_url("NOFILE").await.unwrap_err();
        match err {
            ZoteroError::Api { status, .. } => assert_eq!(status, 404),
            _ => panic!("Expected Api error"),
        }
    }

    // ── Key info tests ────────────────────────────────────────────────

    /// Real response shape (captured from live API, /keys/current):
    /// {"key":"...","userID":16916553,"username":"mattmg","displayName":"","access":{...}}
    #[tokio::test]
    async fn test_get_current_key_info() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/keys/current"))
            .and(header("Zotero-API-Version", "3"))
            .and(header("Zotero-API-Key", "test-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(
                    r#"{"key":"test-key","userID":12345,"username":"testuser","displayName":"","access":{"user":{"library":true,"files":true,"notes":true},"groups":{"all":{"library":true,"write":false}}}}"#,
                ),
            )
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let info = client.get_current_key_info().await.unwrap();
        assert_eq!(info["userID"], 12345);
        assert_eq!(info["username"], "testuser");
        assert_eq!(info["access"]["user"]["library"], true);
    }

    #[tokio::test]
    async fn test_get_current_key_info_forbidden() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/keys/current"))
            .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let err = client.get_current_key_info().await.unwrap_err();
        match err {
            ZoteroError::Api { status, .. } => assert_eq!(status, 403),
            _ => panic!("Expected Api error"),
        }
    }

    // ── Write fixtures ────────────────────────────────────────────────

    fn item_write_response_json(key: &str) -> String {
        format!(
            r#"{{"successful":{{"0":{{"key":"{key}","version":1,"library":{{"type":"user","id":1,"name":"test","links":{{}}}},"links":{{}},"meta":{{}},"data":{{"key":"{key}","version":1,"itemType":"note","note":"test"}}}}}},"unchanged":{{}},"failed":{{}}}}"#
        )
    }

    fn collection_write_response_json(key: &str) -> String {
        format!(
            r#"{{"successful":{{"0":{{"key":"{key}","version":1,"library":{{"type":"user","id":1,"name":"test","links":{{}}}},"links":{{}},"meta":{{"numCollections":0,"numItems":0}},"data":{{"key":"{key}","version":1,"name":"Test Collection","parentCollection":false,"relations":{{}}}}}}}},"unchanged":{{}},"failed":{{}}}}"#
        )
    }

    fn search_write_response_json(key: &str) -> String {
        format!(
            r#"{{"successful":{{"0":{{"key":"{key}","version":1,"library":{{"type":"user","id":1,"name":"test","links":{{}}}},"links":{{}},"data":{{"key":"{key}","version":1,"name":"Test Search","conditions":[]}}}}}},"unchanged":{{}},"failed":{{}}}}"#
        )
    }

    // ── Item write tests ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_create_items() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/users/12345/items"))
            .and(header("Zotero-API-Version", "3"))
            .and(header("Zotero-API-Key", "test-key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(item_write_response_json("NEW12345"))
                    .insert_header("Last-Modified-Version", "101"),
            )
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client
            .create_items(vec![serde_json::json!({"itemType": "note", "note": "test"})])
            .await
            .unwrap();
        assert!(resp.is_ok());
        assert_eq!(resp.successful_keys(), vec!["NEW12345"]);
        assert!(resp.unchanged.is_empty());
        assert!(resp.failed.is_empty());
    }

    #[tokio::test]
    async fn test_create_items_partial_failure() {
        let server = MockServer::start().await;
        let body = r#"{"successful":{"0":{"key":"OK123456","version":1,"library":{"type":"user","id":1,"name":"test","links":{}},"links":{},"meta":{},"data":{"key":"OK123456","version":1,"itemType":"note"}}},"unchanged":{},"failed":{"1":{"key":null,"code":400,"message":"Invalid item type"}}}"#;
        Mock::given(method("POST"))
            .and(path("/users/12345/items"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client
            .create_items(vec![
                serde_json::json!({"itemType": "note"}),
                serde_json::json!({"itemType": "badType"}),
            ])
            .await
            .unwrap();
        assert!(!resp.is_ok());
        assert_eq!(resp.successful.len(), 1);
        assert_eq!(resp.failed.len(), 1);
        assert_eq!(resp.failed["1"].code, 400);
    }

    #[tokio::test]
    async fn test_update_item() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/users/12345/items/ABC12345"))
            .and(header("If-Unmodified-Since-Version", "100"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        client
            .update_item("ABC12345", 100, serde_json::json!({"itemType": "note", "note": "updated"}))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_patch_item() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/users/12345/items/ABC12345"))
            .and(header("If-Unmodified-Since-Version", "100"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        client
            .patch_item("ABC12345", 100, serde_json::json!({"note": "patched"}))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_delete_item() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/users/12345/items/ABC12345"))
            .and(header("If-Unmodified-Since-Version", "100"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        client.delete_item("ABC12345", 100).await.unwrap();
    }

    #[tokio::test]
    async fn test_delete_items() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/users/12345/items"))
            .and(query_param("itemKey", "ABC12345,DEF67890"))
            .and(header("If-Unmodified-Since-Version", "200"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        client
            .delete_items(
                &["ABC12345".to_string(), "DEF67890".to_string()],
                200,
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_write_precondition_failed() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/users/12345/items/ABC12345"))
            .respond_with(
                ResponseTemplate::new(412).set_body_string("Precondition Failed"),
            )
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let err = client
            .update_item("ABC12345", 99, serde_json::json!({"note": "stale"}))
            .await
            .unwrap_err();
        match err {
            ZoteroError::Api { status, .. } => assert_eq!(status, 412),
            _ => panic!("Expected Api error"),
        }
    }

    // ── Collection write tests ────────────────────────────────────────

    #[tokio::test]
    async fn test_create_collections() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/users/12345/collections"))
            .and(header("Zotero-API-Version", "3"))
            .and(header("Zotero-API-Key", "test-key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(collection_write_response_json("NEWCOL01"))
                    .insert_header("Last-Modified-Version", "102"),
            )
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client
            .create_collections(vec![serde_json::json!({"name": "My Collection"})])
            .await
            .unwrap();
        assert!(resp.is_ok());
        assert_eq!(resp.successful_keys(), vec!["NEWCOL01"]);
    }

    #[tokio::test]
    async fn test_update_collection() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/users/12345/collections/COL12345"))
            .and(header("If-Unmodified-Since-Version", "50"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        client
            .update_collection(
                "COL12345",
                50,
                serde_json::json!({"key": "COL12345", "version": 50, "name": "New Name", "parentCollection": false}),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_delete_collection() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/users/12345/collections/COL12345"))
            .and(header("If-Unmodified-Since-Version", "50"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        client.delete_collection("COL12345", 50).await.unwrap();
    }

    #[tokio::test]
    async fn test_delete_collections() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/users/12345/collections"))
            .and(query_param("collectionKey", "COL12345,COL67890"))
            .and(header("If-Unmodified-Since-Version", "200"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        client
            .delete_collections(
                &["COL12345".to_string(), "COL67890".to_string()],
                200,
            )
            .await
            .unwrap();
    }

    // ── Search write tests ────────────────────────────────────────────

    #[tokio::test]
    async fn test_create_searches() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/users/12345/searches"))
            .and(header("Zotero-API-Version", "3"))
            .and(header("Zotero-API-Key", "test-key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(search_write_response_json("SRCH0001"))
                    .insert_header("Last-Modified-Version", "103"),
            )
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        let resp = client
            .create_searches(vec![serde_json::json!({
                "name": "My Search",
                "conditions": [{"condition": "tag", "operator": "is", "value": "test"}]
            })])
            .await
            .unwrap();
        assert!(resp.is_ok());
        assert_eq!(resp.successful_keys(), vec!["SRCH0001"]);
    }

    #[tokio::test]
    async fn test_delete_searches() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/users/12345/searches"))
            .and(query_param("searchKey", "SRCH0001,SRCH0002"))
            .and(header("If-Unmodified-Since-Version", "200"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        client
            .delete_searches(
                &["SRCH0001".to_string(), "SRCH0002".to_string()],
                200,
            )
            .await
            .unwrap();
    }

    // ── Tag write tests ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_tags() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/users/12345/tags"))
            .and(query_param("tag", "test-tag"))
            .and(header("If-Unmodified-Since-Version", "200"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let client = setup_client(&server).await;
        client
            .delete_tags(&["test-tag".to_string()], 200)
            .await
            .unwrap();
    }
}
