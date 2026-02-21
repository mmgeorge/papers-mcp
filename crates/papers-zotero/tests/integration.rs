//! Live integration tests against the Zotero test library.
//!
//! Requires `ZOTERO_TEST_USER_ID` and `ZOTERO_TEST_API_KEY` to be set.
//! Every test creates its own data, verifies it via the read endpoints under
//! test, then cleans up — the test library stays empty between runs.
//!
//! **Serial vs parallel:** Tests that call `delete_items`, `delete_collections`,
//! `delete_searches`, or `delete_tags` (which require the library-level
//! `If-Unmodified-Since-Version`) are marked `#[serial]`. Tests that only use
//! single-object deletes (by object version) can run in parallel.
//!
//! **Guaranteed cleanup:** Drop guards run async cleanup via `block_in_place`
//! so cleanup fires even when a test assertion panics.

use papers_zotero::{
    CollectionListParams, DeletedParams, FulltextParams, ItemListParams, TagListParams, ZoteroClient,
};
use serial_test::serial;

// ── Infrastructure ────────────────────────────────────────────────────

fn client() -> ZoteroClient {
    let user_id =
        std::env::var("ZOTERO_TEST_USER_ID").expect("ZOTERO_TEST_USER_ID must be set for live tests");
    let api_key =
        std::env::var("ZOTERO_TEST_API_KEY").expect("ZOTERO_TEST_API_KEY must be set for live tests");
    ZoteroClient::new(user_id, api_key)
}

/// Current library version — required for multi-object deletes.
async fn library_version(c: &ZoteroClient) -> u64 {
    c.list_items(&ItemListParams::builder().limit(0).build())
        .await
        .unwrap()
        .last_modified_version
        .unwrap_or(0)
}

// Drop guards — guarantee cleanup even on panic.

struct ItemCleanup(Vec<String>);
impl ItemCleanup {
    fn new(keys: impl IntoIterator<Item = String>) -> Self { Self(keys.into_iter().collect()) }
}
impl Drop for ItemCleanup {
    fn drop(&mut self) {
        let keys = std::mem::take(&mut self.0);
        if keys.is_empty() { return; }
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let c = client();
                for key in &keys {
                    if let Ok(item) = c.get_item(key).await {
                        let _ = c.delete_item(key, item.version).await;
                    }
                }
            })
        });
    }
}

struct CollectionCleanup(Vec<String>);
impl CollectionCleanup {
    fn new(keys: impl IntoIterator<Item = String>) -> Self { Self(keys.into_iter().collect()) }
}
impl Drop for CollectionCleanup {
    fn drop(&mut self) {
        let keys = std::mem::take(&mut self.0);
        if keys.is_empty() { return; }
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let c = client();
                for key in &keys {
                    if let Ok(coll) = c.get_collection(key).await {
                        let _ = c.delete_collection(key, coll.version).await;
                    }
                }
            })
        });
    }
}

struct SearchCleanup(Vec<String>);
impl SearchCleanup {
    fn new(keys: impl IntoIterator<Item = String>) -> Self { Self(keys.into_iter().collect()) }
}
impl Drop for SearchCleanup {
    fn drop(&mut self) {
        let keys = std::mem::take(&mut self.0);
        if keys.is_empty() { return; }
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let c = client();
                let lv = library_version(&c).await;
                let _ = c.delete_searches(&keys, lv).await;
            })
        });
    }
}

struct TagCleanup(Vec<String>);
impl TagCleanup {
    fn new(tags: impl IntoIterator<Item = String>) -> Self { Self(tags.into_iter().collect()) }
}
impl Drop for TagCleanup {
    fn drop(&mut self) {
        let tags = std::mem::take(&mut self.0);
        if tags.is_empty() { return; }
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let c = client();
                let lv = library_version(&c).await;
                let _ = c.delete_tags(&tags, lv).await;
            })
        });
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

/// Create a standalone note. Returns (key, version).
async fn create_note(c: &ZoteroClient, note: &str, tags: &[&str]) -> (String, u64) {
    let tag_values: Vec<_> = tags.iter().map(|t| serde_json::json!({"tag": t})).collect();
    let resp = c
        .create_items(vec![serde_json::json!({
            "itemType": "note",
            "note": note,
            "tags": tag_values,
            "collections": [],
            "relations": {}
        })])
        .await
        .unwrap();
    assert!(resp.is_ok(), "create_note failed: {:?}", resp.failed);
    let key = resp.successful_keys()[0].to_string();
    let version = resp.successful["0"]["version"].as_u64().unwrap();
    (key, version)
}

/// Create a journal article. Returns (key, version).
async fn create_article(c: &ZoteroClient, title: &str) -> (String, u64) {
    let resp = c
        .create_items(vec![serde_json::json!({
            "itemType": "journalArticle",
            "title": title,
            "tags": [],
            "collections": [],
            "relations": {}
        })])
        .await
        .unwrap();
    assert!(resp.is_ok(), "create_article failed: {:?}", resp.failed);
    let key = resp.successful_keys()[0].to_string();
    let version = resp.successful["0"]["version"].as_u64().unwrap();
    (key, version)
}

/// Create a collection. Returns (key, version).
async fn create_collection(c: &ZoteroClient, name: &str) -> (String, u64) {
    let resp = c
        .create_collections(vec![serde_json::json!({"name": name})])
        .await
        .unwrap();
    assert!(resp.is_ok(), "create_collection failed: {:?}", resp.failed);
    let key = resp.successful_keys()[0].to_string();
    let version = resp.successful["0"]["version"].as_u64().unwrap();
    (key, version)
}

// ════════════════════════════════════════════════════════════════════
// Key / Auth  (get_key_info, get_current_key_info)
// ════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_get_key_info_has_user_id() {
    let info = client().get_key_info().await.unwrap();
    assert!(info.get("userID").is_some() || info.get("key").is_some());
}

#[tokio::test]
async fn test_get_key_info_user_id_matches_env() {
    let expected = std::env::var("ZOTERO_TEST_USER_ID").unwrap();
    let info = client().get_key_info().await.unwrap();
    let got = info["userID"].as_u64().unwrap().to_string();
    assert_eq!(got, expected);
}

#[tokio::test]
async fn test_get_key_info_has_access_field() {
    let info = client().get_key_info().await.unwrap();
    assert!(info.get("access").is_some());
}

#[tokio::test]
async fn test_get_current_key_info_has_required_fields() {
    let info = client().get_current_key_info().await.unwrap();
    assert!(info.get("userID").is_some());
    assert!(info.get("username").is_some());
    assert!(info.get("access").is_some());
}

#[tokio::test]
async fn test_get_current_key_info_user_id_matches_env() {
    let expected = std::env::var("ZOTERO_TEST_USER_ID").unwrap();
    let info = client().get_current_key_info().await.unwrap();
    let got = info["userID"].as_u64().unwrap().to_string();
    assert_eq!(got, expected);
}

#[tokio::test]
async fn test_get_current_key_info_access_has_user_section() {
    let info = client().get_current_key_info().await.unwrap();
    assert!(info["access"].get("user").is_some() || info["access"].get("groups").is_some());
}

// ════════════════════════════════════════════════════════════════════
// Settings  (get_settings, get_setting)
// ════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_get_settings_has_last_modified_version() {
    let resp = client().get_settings().await.unwrap();
    assert!(resp.last_modified_version.is_some());
}

#[tokio::test]
async fn test_get_settings_version_is_nonzero() {
    let resp = client().get_settings().await.unwrap();
    assert!(resp.last_modified_version.unwrap_or(0) > 0);
}

#[tokio::test]
async fn test_get_settings_tag_colors_structure_if_present() {
    let resp = client().get_settings().await.unwrap();
    if let Some(tc) = resp.data.get("tagColors") {
        assert!(tc.value.is_array());
        assert!(tc.version > 0);
        for entry in tc.value.as_array().unwrap() {
            assert!(entry.get("name").is_some());
            assert!(entry.get("color").is_some());
        }
    }
}

#[tokio::test]
async fn test_get_setting_tag_colors_or_404() {
    match client().get_setting("tagColors").await {
        Ok(resp) => {
            assert!(resp.last_modified_version.is_some());
            assert!(resp.data.value.is_array());
            assert!(resp.data.version > 0);
        }
        Err(papers_zotero::ZoteroError::Api { status, .. }) if status == 404 => {}
        Err(e) => panic!("Unexpected error: {e}"),
    }
}

#[tokio::test]
async fn test_get_setting_nonexistent_returns_404() {
    let err = client().get_setting("nonExistentSetting_xyzzy").await.unwrap_err();
    match err {
        papers_zotero::ZoteroError::Api { status, .. } => assert_eq!(status, 404),
        e => panic!("Expected 404, got: {e}"),
    }
}

#[tokio::test]
async fn test_get_setting_returns_versioned_response() {
    // Any setting that exists should have a versioned response
    match client().get_setting("tagColors").await {
        Ok(resp) => assert!(resp.last_modified_version.is_some()),
        Err(papers_zotero::ZoteroError::Api { status, .. }) if status == 404 => {}
        Err(e) => panic!("Unexpected error: {e}"),
    }
}

// ════════════════════════════════════════════════════════════════════
// Groups  (list_groups)
// ════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_list_groups_responds_ok() {
    let resp = client().list_groups().await.unwrap();
    assert!(resp.total_results.is_some());
}

#[tokio::test]
async fn test_list_groups_has_paged_response() {
    // Groups endpoint does not return Last-Modified-Version; verify items is a vec.
    let resp = client().list_groups().await.unwrap();
    let _ = resp.items.len(); // just verify it deserializes
}

// ════════════════════════════════════════════════════════════════════
// Deleted  (get_deleted)
// ════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_get_deleted_since_zero_has_version() {
    let params = DeletedParams::builder().since(0u64).build();
    let resp = client().get_deleted(&params).await.unwrap();
    assert!(resp.last_modified_version.is_some());
}

#[tokio::test]
async fn test_get_deleted_all_collection_fields_present() {
    let params = DeletedParams::builder().since(0u64).build();
    let resp = client().get_deleted(&params).await.unwrap();
    // All array fields must be present (even if empty)
    let _ = resp.data.items.len();
    let _ = resp.data.collections.len();
    let _ = resp.data.searches.len();
    let _ = resp.data.tags.len();
    let _ = resp.data.settings.len();
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_get_deleted_shows_deleted_item() {
    let c = client();
    let (key, _) = create_note(&c, "<p>delete me</p>", &[]).await;
    let item = c.get_item(&key).await.unwrap();
    let before_version = item.version;
    c.delete_item(&key, before_version).await.unwrap();

    // After deletion, the key should appear in get_deleted
    let params = DeletedParams::builder().since(0u64).build();
    let resp = c.get_deleted(&params).await.unwrap();
    assert!(resp.data.items.iter().any(|k| k == &key));
}

// ════════════════════════════════════════════════════════════════════
// Fulltext  (list_fulltext_versions, get_item_fulltext)
// ════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_list_fulltext_versions_has_version_header() {
    let resp = client().list_fulltext_versions(&FulltextParams::default()).await.unwrap();
    assert!(resp.last_modified_version.is_some());
}

#[tokio::test]
async fn test_list_fulltext_versions_values_are_positive() {
    let resp = client().list_fulltext_versions(&FulltextParams::default()).await.unwrap();
    for (key, version) in &resp.data {
        assert!(!key.is_empty());
        assert!(*version > 0, "version for {key} should be > 0");
    }
}

#[tokio::test]
async fn test_list_fulltext_versions_since_current_is_empty() {
    let all = client().list_fulltext_versions(&FulltextParams::default()).await.unwrap();
    let current = all.last_modified_version.unwrap_or(0);
    let resp = client()
        .list_fulltext_versions(&FulltextParams::builder().since(current).build())
        .await
        .unwrap();
    assert!(resp.data.is_empty(), "no new fulltext since current version");
}

#[tokio::test]
async fn test_get_item_fulltext_if_available() {
    // Fulltext is only available when Zotero desktop has indexed a PDF.
    // This test is defensive: it only asserts structure when data is present.
    let versions = client().list_fulltext_versions(&FulltextParams::default()).await.unwrap();
    if let Some((key, _)) = versions.data.iter().next() {
        let resp = client().get_item_fulltext(key).await.unwrap();
        assert!(!resp.data.content.is_empty());
        assert!(resp.last_modified_version.is_some());
        assert!(resp.data.indexed_pages.is_some() || resp.data.indexed_chars.is_some());
    }
}

// ════════════════════════════════════════════════════════════════════
// File download / view  (download_item_file, get_item_file_view,
//                        get_item_file_view_url)  — error cases only
// ════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_download_item_file_not_found() {
    let err = client().download_item_file("NOTFOUND").await.unwrap_err();
    match err {
        papers_zotero::ZoteroError::Api { status, .. } => {
            assert!(status == 400 || status == 404, "Expected 400 or 404, got {status}")
        }
        e => panic!("Expected API error, got: {e}"),
    }
}

#[tokio::test]
async fn test_get_item_file_view_not_found() {
    let err = client().get_item_file_view("NOTFOUND").await.unwrap_err();
    match err {
        papers_zotero::ZoteroError::Api { status, .. } => {
            assert!(status == 400 || status == 404, "Expected 400 or 404, got {status}")
        }
        e => panic!("Expected API error, got: {e}"),
    }
}

#[tokio::test]
async fn test_get_item_file_view_url_not_found() {
    let err = client().get_item_file_view_url("NOTFOUND").await.unwrap_err();
    match err {
        papers_zotero::ZoteroError::Api { status, .. } => {
            assert!(status == 400 || status == 404, "Expected 400 or 404, got {status}")
        }
        e => panic!("Expected API error, got: {e}"),
    }
}

// ════════════════════════════════════════════════════════════════════
// list_items
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_items_shows_created_item() {
    let c = client();
    let (key, _) = create_note(&c, "<p>list items test</p>", &[]).await;
    let _cleanup = ItemCleanup::new([key.clone()]);

    let params = ItemListParams::builder().limit(50).build();
    let resp = c.list_items(&params).await.unwrap();
    assert!(resp.items.iter().any(|i| i.key == key), "item missing from list_items");
    assert!(resp.total_results.is_some());
    assert!(resp.last_modified_version.is_some());
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_items_with_type_filter() {
    let c = client();
    let (key, _) = create_article(&c, "Type filter test article").await;
    let _cleanup = ItemCleanup::new([key.clone()]);

    let params = ItemListParams::builder().item_type("journalArticle").limit(50).build();
    let resp = c.list_items(&params).await.unwrap();
    assert!(resp.items.iter().any(|i| i.key == key));
    // All returned items should be journalArticle
    for item in &resp.items {
        assert_eq!(item.data.item_type.as_str(), "journalArticle");
    }
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_items_with_search_query() {
    let c = client();
    let (key, _) = create_article(&c, "Unique Query Match XYZ789").await;
    let _cleanup = ItemCleanup::new([key.clone()]);

    let params = ItemListParams::builder().q("Unique Query Match XYZ789").limit(10).build();
    let resp = c.list_items(&params).await.unwrap();
    assert!(resp.items.iter().any(|i| i.key == key), "search query should find the item");
}

#[tokio::test]
async fn test_list_items_limit_zero_returns_version() {
    // Zotero enforces a minimum limit of 1; limit=0 is treated as 1.
    // The important thing is that last_modified_version is returned.
    let params = ItemListParams::builder().limit(0).build();
    let resp = client().list_items(&params).await.unwrap();
    assert!(resp.total_results.is_some());
    assert!(resp.last_modified_version.is_some());
}

// ════════════════════════════════════════════════════════════════════
// list_top_items
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_top_items_shows_standalone_note() {
    let c = client();
    let (key, _) = create_note(&c, "<p>top item</p>", &[]).await;
    let _cleanup = ItemCleanup::new([key.clone()]);

    let resp = c.list_top_items(&ItemListParams::default()).await.unwrap();
    assert!(resp.items.iter().any(|i| i.key == key), "standalone note should be in top items");
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_top_items_excludes_child_note() {
    let c = client();
    // Create parent article + child note
    let (parent_key, _) = create_article(&c, "Parent for child note test").await;
    let _parent_cleanup = ItemCleanup::new([parent_key.clone()]);

    let resp = c
        .create_items(vec![serde_json::json!({
            "itemType": "note",
            "parentItem": parent_key,
            "note": "<p>Child note</p>"
        })])
        .await
        .unwrap();
    assert!(resp.is_ok());
    let child_key = resp.successful_keys()[0].to_string();
    let _child_cleanup = ItemCleanup::new([child_key.clone()]);

    // Child note should NOT appear in list_top_items
    let top = c.list_top_items(&ItemListParams::default()).await.unwrap();
    assert!(
        !top.items.iter().any(|i| i.key == child_key),
        "child note should not appear in top items"
    );
    // Parent should appear
    assert!(top.items.iter().any(|i| i.key == parent_key), "parent article should be in top items");
}

#[tokio::test]
async fn test_list_top_items_limit_zero() {
    let params = ItemListParams::builder().limit(0).build();
    let resp = client().list_top_items(&params).await.unwrap();
    assert!(resp.last_modified_version.is_some());
}

// ════════════════════════════════════════════════════════════════════
// list_trash_items
// ════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_list_trash_items_responds_ok() {
    let resp = client().list_trash_items(&ItemListParams::default()).await.unwrap();
    assert!(resp.total_results.is_some());
}

#[tokio::test]
async fn test_list_trash_items_limit_zero() {
    let params = ItemListParams::builder().limit(0).build();
    let resp = client().list_trash_items(&params).await.unwrap();
    assert!(resp.items.is_empty());
    assert!(resp.last_modified_version.is_some());
}

// ════════════════════════════════════════════════════════════════════
// get_item
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_get_item_fields_match_created() {
    let c = client();
    let (key, _) = create_note(&c, "<p>get item test</p>", &["get-item-tag"]).await;
    let _cleanup = ItemCleanup::new([key.clone()]);

    let item = c.get_item(&key).await.unwrap();
    assert_eq!(item.key, key);
    assert_eq!(item.data.note.as_deref(), Some("<p>get item test</p>"));
    assert_eq!(item.data.item_type.as_str(), "note");
    assert!(item.version > 0);
    assert!(item.data.tags.iter().any(|t| t.tag == "get-item-tag"));
}

#[tokio::test]
async fn test_get_item_not_found_returns_404() {
    let err = client().get_item("NOTFOUND").await.unwrap_err();
    match err {
        papers_zotero::ZoteroError::Api { status, .. } => assert_eq!(status, 404),
        e => panic!("Expected 404, got: {e}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_get_item_has_library_and_links() {
    let c = client();
    let (key, _) = create_note(&c, "<p>library check</p>", &[]).await;
    let _cleanup = ItemCleanup::new([key.clone()]);

    let item = c.get_item(&key).await.unwrap();
    // library should identify the test user
    assert!(!item.library.name.is_empty());
    // links should be populated
    assert!(!item.links.is_empty());
}

// ════════════════════════════════════════════════════════════════════
// list_item_children
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_item_children_empty_for_standalone_note() {
    let c = client();
    let (key, _) = create_note(&c, "<p>childless note</p>", &[]).await;
    let _cleanup = ItemCleanup::new([key.clone()]);

    let children = c.list_item_children(&key, &ItemListParams::default()).await.unwrap();
    assert!(children.items.is_empty());
    assert!(children.total_results.is_some());
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_item_children_shows_child_note() {
    let c = client();
    let (parent_key, _) = create_article(&c, "Parent with child").await;
    let _parent_cleanup = ItemCleanup::new([parent_key.clone()]);

    let resp = c
        .create_items(vec![serde_json::json!({
            "itemType": "note",
            "parentItem": parent_key,
            "note": "<p>I am a child</p>"
        })])
        .await
        .unwrap();
    assert!(resp.is_ok());
    let child_key = resp.successful_keys()[0].to_string();
    let _child_cleanup = ItemCleanup::new([child_key.clone()]);

    let children = c.list_item_children(&parent_key, &ItemListParams::default()).await.unwrap();
    assert!(children.items.iter().any(|i| i.key == child_key));
    assert_eq!(children.items[0].data.item_type.as_str(), "note");
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_item_children_limit_param() {
    let c = client();
    let (key, _) = create_note(&c, "<p>limit test</p>", &[]).await;
    let _cleanup = ItemCleanup::new([key.clone()]);

    let params = ItemListParams::builder().limit(1).build();
    let children = c.list_item_children(&key, &params).await.unwrap();
    assert!(children.items.len() <= 1);
}

// ════════════════════════════════════════════════════════════════════
// list_publication_items
// ════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_list_publication_items_responds_ok() {
    // Publication items can't be created via the API; this just verifies the
    // endpoint is reachable and returns a well-formed response.
    let resp = client().list_publication_items(&ItemListParams::default()).await.unwrap();
    assert!(resp.total_results.is_some());
}

#[tokio::test]
async fn test_list_publication_items_limit_zero() {
    let params = ItemListParams::builder().limit(0).build();
    let resp = client().list_publication_items(&params).await.unwrap();
    assert!(resp.items.is_empty());
    assert!(resp.last_modified_version.is_some());
}

// ════════════════════════════════════════════════════════════════════
// list_collection_items / list_collection_top_items
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_collection_items_shows_item() {
    let c = client();
    let (coll_key, _) = create_collection(&c, "Coll items test").await;
    let _coll_cleanup = CollectionCleanup::new([coll_key.clone()]);

    let resp = c
        .create_items(vec![serde_json::json!({
            "itemType": "note",
            "note": "<p>in collection</p>",
            "collections": [coll_key],
            "relations": {},
            "tags": []
        })])
        .await
        .unwrap();
    assert!(resp.is_ok());
    let item_key = resp.successful_keys()[0].to_string();
    let _item_cleanup = ItemCleanup::new([item_key.clone()]);

    let items = c
        .list_collection_items(&coll_key, &ItemListParams::default())
        .await
        .unwrap();
    assert!(items.items.iter().any(|i| i.key == item_key));
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_collection_items_empty_for_new_collection() {
    let c = client();
    let (coll_key, _) = create_collection(&c, "Empty coll test").await;
    let _cleanup = CollectionCleanup::new([coll_key.clone()]);

    let items = c
        .list_collection_items(&coll_key, &ItemListParams::default())
        .await
        .unwrap();
    assert!(items.items.is_empty());
    assert!(items.total_results.is_some());
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_collection_items_limit_zero() {
    let c = client();
    let (coll_key, _) = create_collection(&c, "Coll limit zero test").await;
    let _cleanup = CollectionCleanup::new([coll_key.clone()]);

    let params = ItemListParams::builder().limit(0).build();
    let items = c.list_collection_items(&coll_key, &params).await.unwrap();
    assert!(items.items.is_empty());
    assert!(items.last_modified_version.is_some());
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_collection_top_items_shows_item() {
    let c = client();
    let (coll_key, _) = create_collection(&c, "Coll top items test").await;
    let _coll_cleanup = CollectionCleanup::new([coll_key.clone()]);

    let resp = c
        .create_items(vec![serde_json::json!({
            "itemType": "note",
            "note": "<p>top in collection</p>",
            "collections": [coll_key],
            "relations": {},
            "tags": []
        })])
        .await
        .unwrap();
    assert!(resp.is_ok());
    let item_key = resp.successful_keys()[0].to_string();
    let _item_cleanup = ItemCleanup::new([item_key.clone()]);

    let top = c
        .list_collection_top_items(&coll_key, &ItemListParams::default())
        .await
        .unwrap();
    assert!(top.items.iter().any(|i| i.key == item_key));
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_collection_top_items_empty_collection() {
    let c = client();
    let (coll_key, _) = create_collection(&c, "Empty top items coll").await;
    let _cleanup = CollectionCleanup::new([coll_key.clone()]);

    let top = c
        .list_collection_top_items(&coll_key, &ItemListParams::default())
        .await
        .unwrap();
    assert!(top.items.is_empty());
}

// ════════════════════════════════════════════════════════════════════
// list_collections / list_top_collections / get_collection /
// list_subcollections
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_collections_shows_created() {
    let c = client();
    let (key, _) = create_collection(&c, "List collections test").await;
    let _cleanup = CollectionCleanup::new([key.clone()]);

    let resp = c.list_collections(&CollectionListParams::default()).await.unwrap();
    assert!(resp.items.iter().any(|c| c.key == key));
    assert!(resp.total_results.is_some());
    assert!(resp.last_modified_version.is_some());
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_collections_with_limit_param() {
    let c = client();
    let (key, _) = create_collection(&c, "Limit param coll").await;
    let _cleanup = CollectionCleanup::new([key.clone()]);

    let params = CollectionListParams::builder().limit(1).build();
    let resp = c.list_collections(&params).await.unwrap();
    assert!(resp.items.len() <= 1);
    assert!(resp.total_results.is_some());
}

#[tokio::test]
async fn test_list_collections_limit_zero() {
    let params = CollectionListParams::builder().limit(0).build();
    let resp = client().list_collections(&params).await.unwrap();
    assert!(resp.last_modified_version.is_some());
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_top_collections_shows_top_level() {
    let c = client();
    let (key, _) = create_collection(&c, "Top-level coll").await;
    let _cleanup = CollectionCleanup::new([key.clone()]);

    let resp = c.list_top_collections(&CollectionListParams::default()).await.unwrap();
    assert!(resp.items.iter().any(|c| c.key == key));
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_top_collections_excludes_subcollection() {
    let c = client();
    let (parent_key, _) = create_collection(&c, "Parent coll for top test").await;
    let _parent_cleanup = CollectionCleanup::new([parent_key.clone()]);

    // Create a subcollection
    let resp = c
        .create_collections(vec![serde_json::json!({
            "name": "Child coll",
            "parentCollection": parent_key
        })])
        .await
        .unwrap();
    assert!(resp.is_ok());
    let child_key = resp.successful_keys()[0].to_string();
    let _child_cleanup = CollectionCleanup::new([child_key.clone()]);

    let top = c.list_top_collections(&CollectionListParams::default()).await.unwrap();
    assert!(top.items.iter().any(|c| c.key == parent_key), "parent should be in top");
    assert!(
        !top.items.iter().any(|c| c.key == child_key),
        "child should NOT be in top collections"
    );
}

#[tokio::test]
async fn test_list_top_collections_limit_zero() {
    let params = CollectionListParams::builder().limit(0).build();
    let resp = client().list_top_collections(&params).await.unwrap();
    assert!(resp.last_modified_version.is_some());
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_get_collection_fields_match_created() {
    let c = client();
    let (key, _) = create_collection(&c, "Get collection test").await;
    let _cleanup = CollectionCleanup::new([key.clone()]);

    let coll = c.get_collection(&key).await.unwrap();
    assert_eq!(coll.key, key);
    assert_eq!(coll.data.name, "Get collection test");
    assert!(coll.version > 0);
    // Top-level: parentCollection should be false
    assert!(coll.data.parent_key().is_none());
}

#[tokio::test]
async fn test_get_collection_not_found_returns_404() {
    let err = client().get_collection("NOTFOUND").await.unwrap_err();
    match err {
        papers_zotero::ZoteroError::Api { status, .. } => assert_eq!(status, 404),
        e => panic!("Expected 404, got: {e}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_get_collection_subcollection_has_parent_key() {
    let c = client();
    let (parent_key, _) = create_collection(&c, "Parent for subcoll get test").await;
    let _parent_cleanup = CollectionCleanup::new([parent_key.clone()]);

    let resp = c
        .create_collections(vec![serde_json::json!({
            "name": "Child for get test",
            "parentCollection": parent_key
        })])
        .await
        .unwrap();
    assert!(resp.is_ok());
    let child_key = resp.successful_keys()[0].to_string();
    let _child_cleanup = CollectionCleanup::new([child_key.clone()]);

    let coll = c.get_collection(&child_key).await.unwrap();
    assert_eq!(coll.data.parent_key(), Some(parent_key.as_str()));
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_subcollections_shows_child() {
    let c = client();
    let (parent_key, _) = create_collection(&c, "Parent for subcoll list").await;
    let _parent_cleanup = CollectionCleanup::new([parent_key.clone()]);

    let resp = c
        .create_collections(vec![serde_json::json!({
            "name": "Subcollection A",
            "parentCollection": parent_key
        })])
        .await
        .unwrap();
    assert!(resp.is_ok());
    let child_key = resp.successful_keys()[0].to_string();
    let _child_cleanup = CollectionCleanup::new([child_key.clone()]);

    let subs = c.list_subcollections(&parent_key, &CollectionListParams::default()).await.unwrap();
    assert!(subs.items.iter().any(|c| c.key == child_key));
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_subcollections_empty_for_leaf() {
    let c = client();
    let (key, _) = create_collection(&c, "Leaf collection").await;
    let _cleanup = CollectionCleanup::new([key.clone()]);

    let subs = c.list_subcollections(&key, &CollectionListParams::default()).await.unwrap();
    assert!(subs.items.is_empty());
    assert!(subs.total_results.is_some());
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_subcollections_multiple_children() {
    let c = client();
    let (parent_key, _) = create_collection(&c, "Parent with two children").await;
    let _parent_cleanup = CollectionCleanup::new([parent_key.clone()]);

    let resp = c
        .create_collections(vec![
            serde_json::json!({"name": "Child 1", "parentCollection": parent_key}),
            serde_json::json!({"name": "Child 2", "parentCollection": parent_key}),
        ])
        .await
        .unwrap();
    assert!(resp.is_ok());
    let child_keys: Vec<String> = resp.successful_keys().into_iter().map(String::from).collect();
    let _child_cleanup = CollectionCleanup::new(child_keys.clone());

    let subs = c.list_subcollections(&parent_key, &CollectionListParams::default()).await.unwrap();
    assert_eq!(subs.items.len(), 2);
    for ck in &child_keys {
        assert!(subs.items.iter().any(|c| &c.key == ck));
    }
}

// ════════════════════════════════════════════════════════════════════
// list_searches / get_search
// ════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_list_searches_responds_ok() {
    let resp = client().list_searches().await.unwrap();
    assert!(resp.total_results.is_some());
    assert!(resp.last_modified_version.is_some());
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_searches_shows_created_search() {
    let c = client();
    let resp = c
        .create_searches(vec![serde_json::json!({
            "name": "List searches test",
            "conditions": [{"condition": "tag", "operator": "is", "value": "test-x"}]
        })])
        .await
        .unwrap();
    assert!(resp.is_ok());
    let key = resp.successful_keys()[0].to_string();
    let _cleanup = SearchCleanup::new([key.clone()]);

    let searches = c.list_searches().await.unwrap();
    assert!(searches.items.iter().any(|s| s.key == key));
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_get_search_fields_match_created() {
    let c = client();
    let resp = c
        .create_searches(vec![serde_json::json!({
            "name": "Get search test",
            "conditions": [{"condition": "title", "operator": "contains", "value": "rust"}]
        })])
        .await
        .unwrap();
    assert!(resp.is_ok());
    let key = resp.successful_keys()[0].to_string();
    let _cleanup = SearchCleanup::new([key.clone()]);

    let search = c.get_search(&key).await.unwrap();
    assert_eq!(search.key, key);
    assert_eq!(search.data.name, "Get search test");
    assert_eq!(search.data.conditions.len(), 1);
    assert_eq!(search.data.conditions[0].condition, "title");
    assert_eq!(search.data.conditions[0].operator, "contains");
    assert_eq!(search.data.conditions[0].value, "rust");
}

#[tokio::test]
async fn test_get_search_not_found_returns_404() {
    let err = client().get_search("NOTFOUND").await.unwrap_err();
    match err {
        papers_zotero::ZoteroError::Api { status, .. } => assert_eq!(status, 404),
        e => panic!("Expected 404, got: {e}"),
    }
}

// ════════════════════════════════════════════════════════════════════
// list_tags / get_tag
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_tags_shows_created_tag() {
    let c = client();
    let (key, _) = create_note(&c, "<p>tagged</p>", &["unique-tag-xyz-001"]).await;
    let _item = ItemCleanup::new([key.clone()]);
    let _tag = TagCleanup::new(["unique-tag-xyz-001".to_string()]);

    let resp = c.list_tags(&TagListParams::builder().limit(100).build()).await.unwrap();
    assert!(resp.items.iter().any(|t| t.tag == "unique-tag-xyz-001"));
    assert!(resp.total_results.is_some());
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_tags_with_limit_param() {
    let c = client();
    let (key, _) = create_note(&c, "<p>tag limit</p>", &["tag-limit-test-001"]).await;
    let _item = ItemCleanup::new([key.clone()]);
    let _tag = TagCleanup::new(["tag-limit-test-001".to_string()]);

    let resp = c.list_tags(&TagListParams::builder().limit(1).build()).await.unwrap();
    assert!(resp.items.len() <= 1);
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_tags_has_version_header() {
    let c = client();
    let (key, _) = create_note(&c, "<p>tag version</p>", &["tag-version-test"]).await;
    let _item = ItemCleanup::new([key.clone()]);
    let _tag = TagCleanup::new(["tag-version-test".to_string()]);

    let resp = c.list_tags(&TagListParams::default()).await.unwrap();
    assert!(resp.last_modified_version.is_some());
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_get_tag_returns_array_with_one_element() {
    let c = client();
    let (key, _) = create_note(&c, "<p>get tag</p>", &["get-tag-test-001"]).await;
    let _item = ItemCleanup::new([key.clone()]);
    let _tag = TagCleanup::new(["get-tag-test-001".to_string()]);

    // get_tag returns a PagedResponse<Tag> (array)
    let resp = c.get_tag("get-tag-test-001").await.unwrap();
    assert!(!resp.items.is_empty(), "known tag should return at least one element");
    assert_eq!(resp.items[0].tag, "get-tag-test-001");
}

#[tokio::test]
async fn test_get_tag_unknown_name_returns_empty() {
    // Unknown tag returns 404 or an empty array — handle both
    match client().get_tag("__definitely_does_not_exist_xyzzy__").await {
        Ok(resp) => assert!(resp.items.is_empty()),
        Err(papers_zotero::ZoteroError::Api { status, .. }) if status == 404 => {}
        Err(e) => panic!("Unexpected error: {e}"),
    }
}

// ════════════════════════════════════════════════════════════════════
// list_item_tags
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_item_tags_shows_item_tags() {
    let c = client();
    let (key, _) = create_note(&c, "<p>item tags</p>", &["item-tag-a", "item-tag-b"]).await;
    let _item = ItemCleanup::new([key.clone()]);
    let _tags = TagCleanup::new(["item-tag-a".to_string(), "item-tag-b".to_string()]);

    let resp = c.list_item_tags(&key, &TagListParams::default()).await.unwrap();
    assert!(resp.items.iter().any(|t| t.tag == "item-tag-a"));
    assert!(resp.items.iter().any(|t| t.tag == "item-tag-b"));
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_item_tags_empty_for_untagged_item() {
    let c = client();
    let (key, _) = create_note(&c, "<p>untagged</p>", &[]).await;
    let _cleanup = ItemCleanup::new([key.clone()]);

    let resp = c.list_item_tags(&key, &TagListParams::default()).await.unwrap();
    assert!(resp.items.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_item_tags_with_limit() {
    let c = client();
    let (key, _) = create_note(&c, "<p>item tags limit</p>", &["item-limit-tag-1", "item-limit-tag-2"]).await;
    let _item = ItemCleanup::new([key.clone()]);
    let _tags = TagCleanup::new(["item-limit-tag-1".to_string(), "item-limit-tag-2".to_string()]);

    let resp = c.list_item_tags(&key, &TagListParams::builder().limit(1).build()).await.unwrap();
    assert!(resp.items.len() <= 1);
    assert!(resp.total_results.is_some());
}

// ════════════════════════════════════════════════════════════════════
// list_items_tags / list_top_items_tags / list_trash_tags /
// list_publication_tags
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_items_tags_shows_created_tag() {
    let c = client();
    let (key, _) = create_note(&c, "<p>items tags test</p>", &["items-tags-test-001"]).await;
    let _item = ItemCleanup::new([key.clone()]);
    let _tag = TagCleanup::new(["items-tags-test-001".to_string()]);

    let resp = c.list_items_tags(&TagListParams::default()).await.unwrap();
    assert!(resp.items.iter().any(|t| t.tag == "items-tags-test-001"));
}

#[tokio::test]
async fn test_list_items_tags_responds_ok() {
    let resp = client().list_items_tags(&TagListParams::default()).await.unwrap();
    assert!(resp.total_results.is_some());
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_top_items_tags_shows_tag() {
    let c = client();
    // A standalone note is a top-level item
    let (key, _) = create_note(&c, "<p>top items tag test</p>", &["top-items-tag-001"]).await;
    let _item = ItemCleanup::new([key.clone()]);
    let _tag = TagCleanup::new(["top-items-tag-001".to_string()]);

    let resp = c.list_top_items_tags(&TagListParams::default()).await.unwrap();
    assert!(resp.items.iter().any(|t| t.tag == "top-items-tag-001"));
}

#[tokio::test]
async fn test_list_top_items_tags_responds_ok() {
    let resp = client().list_top_items_tags(&TagListParams::default()).await.unwrap();
    assert!(resp.total_results.is_some());
    assert!(resp.last_modified_version.is_some());
}

#[tokio::test]
async fn test_list_trash_tags_responds_ok() {
    let resp = client().list_trash_tags(&TagListParams::default()).await.unwrap();
    assert!(resp.total_results.is_some());
}

#[tokio::test]
async fn test_list_publication_tags_responds_ok() {
    // Known quirk: returns ALL library tags, not just publication tags.
    let resp = client().list_publication_tags(&TagListParams::default()).await.unwrap();
    assert!(resp.total_results.is_some());
}

// ════════════════════════════════════════════════════════════════════
// list_collection_tags / list_collection_items_tags /
// list_collection_top_items_tags
// ════════════════════════════════════════════════════════════════════

/// Shared helper: creates a collection with one tagged item inside it.
/// Returns (coll_key, item_key, tag_name).
async fn make_collection_with_tagged_item(
    c: &ZoteroClient,
    tag: &str,
) -> (String, String) {
    let (coll_key, _) = create_collection(c, "Coll tag test").await;
    let resp = c
        .create_items(vec![serde_json::json!({
            "itemType": "note",
            "note": "<p>tagged in coll</p>",
            "collections": [coll_key],
            "tags": [{"tag": tag}],
            "relations": {}
        })])
        .await
        .unwrap();
    assert!(resp.is_ok());
    let item_key = resp.successful_keys()[0].to_string();
    (coll_key, item_key)
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_collection_tags_responds_ok() {
    // Note: /collections/{key}/tags returns tags on the collection object itself
    // (not tags of items inside it — use list_collection_items_tags for that).
    // Collections normally have no tags of their own, so this returns empty.
    let c = client();
    let (coll_key, item_key) = make_collection_with_tagged_item(&c, "coll-tag-001").await;
    let _coll = CollectionCleanup::new([coll_key.clone()]);
    let _item = ItemCleanup::new([item_key.clone()]);
    let _tag = TagCleanup::new(["coll-tag-001".to_string()]);

    let resp = c.list_collection_tags(&coll_key, &TagListParams::default()).await.unwrap();
    assert!(resp.total_results.is_some());
    // Items may be empty since collections don't have tags of their own
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_collection_tags_empty_for_new_collection() {
    let c = client();
    let (coll_key, _) = create_collection(&c, "Empty coll tags").await;
    let _cleanup = CollectionCleanup::new([coll_key.clone()]);

    let resp = c.list_collection_tags(&coll_key, &TagListParams::default()).await.unwrap();
    assert!(resp.items.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_collection_items_tags_shows_tag() {
    let c = client();
    let (coll_key, item_key) = make_collection_with_tagged_item(&c, "coll-items-tag-001").await;
    let _coll = CollectionCleanup::new([coll_key.clone()]);
    let _item = ItemCleanup::new([item_key.clone()]);
    let _tag = TagCleanup::new(["coll-items-tag-001".to_string()]);

    let resp = c.list_collection_items_tags(&coll_key, &TagListParams::default()).await.unwrap();
    assert!(resp.items.iter().any(|t| t.tag == "coll-items-tag-001"));
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_collection_items_tags_empty_collection() {
    let c = client();
    let (coll_key, _) = create_collection(&c, "Empty coll items tags").await;
    let _cleanup = CollectionCleanup::new([coll_key.clone()]);

    let resp = c.list_collection_items_tags(&coll_key, &TagListParams::default()).await.unwrap();
    assert!(resp.items.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_collection_top_items_tags_shows_tag() {
    let c = client();
    let (coll_key, item_key) = make_collection_with_tagged_item(&c, "coll-top-tag-001").await;
    let _coll = CollectionCleanup::new([coll_key.clone()]);
    let _item = ItemCleanup::new([item_key.clone()]);
    let _tag = TagCleanup::new(["coll-top-tag-001".to_string()]);

    let resp = c.list_collection_top_items_tags(&coll_key, &TagListParams::default()).await.unwrap();
    assert!(resp.items.iter().any(|t| t.tag == "coll-top-tag-001"));
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_list_collection_top_items_tags_empty_collection() {
    let c = client();
    let (coll_key, _) = create_collection(&c, "Empty coll top items tags").await;
    let _cleanup = CollectionCleanup::new([coll_key.clone()]);

    let resp = c.list_collection_top_items_tags(&coll_key, &TagListParams::default()).await.unwrap();
    assert!(resp.items.is_empty());
}

// ════════════════════════════════════════════════════════════════════
// create_items — extra cases
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_create_items_partial_failure() {
    let c = client();
    let resp = c
        .create_items(vec![
            serde_json::json!({"itemType": "note", "note": "<p>good</p>", "tags": [], "collections": [], "relations": {}}),
            serde_json::json!({"itemType": "__invalid__"}),
        ])
        .await
        .unwrap();
    // At least one should fail
    assert!(!resp.is_ok());
    assert_eq!(resp.failed.len(), 1);
    assert!(resp.failed["1"].code >= 400);
    // Clean up the successful item
    let _cleanup = ItemCleanup::new(resp.successful_keys().into_iter().map(String::from));
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_create_items_batch_returns_all_keys() {
    let c = client();
    let resp = c
        .create_items(vec![
            serde_json::json!({"itemType": "note", "note": "<p>batch A</p>", "tags": [], "collections": [], "relations": {}}),
            serde_json::json!({"itemType": "note", "note": "<p>batch B</p>", "tags": [], "collections": [], "relations": {}}),
            serde_json::json!({"itemType": "note", "note": "<p>batch C</p>", "tags": [], "collections": [], "relations": {}}),
        ])
        .await
        .unwrap();
    assert!(resp.is_ok());
    assert_eq!(resp.successful.len(), 3);
    let _cleanup = ItemCleanup::new(resp.successful_keys().into_iter().map(String::from));
}

// ════════════════════════════════════════════════════════════════════
// update_item — extra cases
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_update_item_version_conflict_returns_412() {
    let c = client();
    let (key, _) = create_note(&c, "<p>conflict test</p>", &[]).await;
    let _cleanup = ItemCleanup::new([key.clone()]);

    let err = c
        .update_item(
            &key,
            0, // wrong version
            serde_json::json!({
                "key": key,
                "version": 0,
                "itemType": "note",
                "note": "<p>bad</p>",
                "tags": [],
                "collections": [],
                "relations": {}
            }),
        )
        .await
        .unwrap_err();
    match err {
        papers_zotero::ZoteroError::Api { status, .. } => assert_eq!(status, 412),
        e => panic!("Expected 412, got: {e}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_update_item_bogus_key_succeeds() {
    // Zotero accepts a PUT for a non-existent key (upsert semantics), returning 204.
    // We use a fixed key and clean it up via the drop guard.
    let _cleanup = ItemCleanup::new(["UPSRTITM".to_string()]);
    client()
        .update_item(
            "UPSRTITM",
            0,
            serde_json::json!({
                "key": "UPSRTITM",
                "version": 0,
                "itemType": "note",
                "note": "<p>upsert test</p>",
                "tags": [],
                "collections": [],
                "relations": {}
            }),
        )
        .await
        .unwrap();
}

// ════════════════════════════════════════════════════════════════════
// patch_item — extra cases
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_patch_item_preserves_other_fields() {
    let c = client();
    let (key, _) = create_note(&c, "<p>original</p>", &["preserve-tag"]).await;
    let _cleanup = ItemCleanup::new([key.clone()]);

    let item = c.get_item(&key).await.unwrap();
    let version = item.version;

    // Patch only the note content
    c.patch_item(&key, version, serde_json::json!({"note": "<p>patched</p>"}))
        .await
        .unwrap();

    // Tags should still be present after patch
    let after = c.get_item(&key).await.unwrap();
    assert_eq!(after.data.note.as_deref(), Some("<p>patched</p>"));
    assert!(after.data.tags.iter().any(|t| t.tag == "preserve-tag"));
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_patch_item_version_conflict_returns_412() {
    let c = client();
    let (key, _) = create_note(&c, "<p>patch conflict</p>", &[]).await;
    let _cleanup = ItemCleanup::new([key.clone()]);

    let err = c
        .patch_item(&key, 0, serde_json::json!({"note": "<p>bad</p>"}))
        .await
        .unwrap_err();
    match err {
        papers_zotero::ZoteroError::Api { status, .. } => assert_eq!(status, 412),
        e => panic!("Expected 412, got: {e}"),
    }
}

// ════════════════════════════════════════════════════════════════════
// delete_item — extra cases
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_delete_item_version_conflict_returns_412() {
    let c = client();
    let (key, _) = create_note(&c, "<p>delete conflict</p>", &[]).await;
    let _cleanup = ItemCleanup::new([key.clone()]);

    let err = c.delete_item(&key, 0).await.unwrap_err();
    match err {
        papers_zotero::ZoteroError::Api { status, .. } => assert_eq!(status, 412),
        e => panic!("Expected 412, got: {e}"),
    }
    // _cleanup will delete with correct version
}

#[tokio::test]
async fn test_delete_item_nonexistent_returns_404() {
    // Zotero returns 404 for items that do not exist.
    let err = client().delete_item("NOTFOUND", 0).await.unwrap_err();
    match err {
        papers_zotero::ZoteroError::Api { status, .. } => assert_eq!(status, 404),
        e => panic!("Expected 404, got: {e}"),
    }
}

// ════════════════════════════════════════════════════════════════════
// delete_items — extra cases
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_delete_items_version_conflict_returns_412() {
    let c = client();
    let (key, _) = create_note(&c, "<p>multi delete conflict</p>", &[]).await;
    let _cleanup = ItemCleanup::new([key.clone()]);

    let err = c.delete_items(&[key.clone()], 0).await.unwrap_err();
    match err {
        papers_zotero::ZoteroError::Api { status, .. } => assert_eq!(status, 412),
        e => panic!("Expected 412, got: {e}"),
    }
    // _cleanup deletes via single-item delete with correct version
}

// ════════════════════════════════════════════════════════════════════
// create_collections — extra cases
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_create_collections_batch() {
    let c = client();
    let resp = c
        .create_collections(vec![
            serde_json::json!({"name": "Batch Coll A"}),
            serde_json::json!({"name": "Batch Coll B"}),
        ])
        .await
        .unwrap();
    assert!(resp.is_ok());
    assert_eq!(resp.successful.len(), 2);
    let _cleanup = CollectionCleanup::new(resp.successful_keys().into_iter().map(String::from));
}

// ════════════════════════════════════════════════════════════════════
// update_collection — extra cases
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_update_collection_version_conflict_returns_412() {
    let c = client();
    let (key, _) = create_collection(&c, "Update conflict test").await;
    let _cleanup = CollectionCleanup::new([key.clone()]);

    let err = c
        .update_collection(
            &key,
            0, // wrong version
            serde_json::json!({
                "key": key,
                "version": 0,
                "name": "bad",
                "parentCollection": false,
                "relations": {}
            }),
        )
        .await
        .unwrap_err();
    match err {
        papers_zotero::ZoteroError::Api { status, .. } => assert_eq!(status, 412),
        e => panic!("Expected 412, got: {e}"),
    }
}

// ════════════════════════════════════════════════════════════════════
// delete_collection — extra cases
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_delete_collection_version_conflict_returns_412() {
    let c = client();
    let (key, _) = create_collection(&c, "Delete coll conflict").await;
    let _cleanup = CollectionCleanup::new([key.clone()]);

    let err = c.delete_collection(&key, 0).await.unwrap_err();
    match err {
        papers_zotero::ZoteroError::Api { status, .. } => assert_eq!(status, 412),
        e => panic!("Expected 412, got: {e}"),
    }
}

#[tokio::test]
async fn test_delete_collection_nonexistent_returns_404() {
    // Zotero returns 404 for collections that do not exist.
    let err = client().delete_collection("NOTFOUND", 0).await.unwrap_err();
    match err {
        papers_zotero::ZoteroError::Api { status, .. } => assert_eq!(status, 404),
        e => panic!("Expected 404, got: {e}"),
    }
}

// ════════════════════════════════════════════════════════════════════
// delete_collections — extra cases
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_delete_collections_version_conflict_returns_412() {
    let c = client();
    let (key, _) = create_collection(&c, "Multi delete coll conflict").await;
    let _cleanup = CollectionCleanup::new([key.clone()]);

    let err = c.delete_collections(&[key.clone()], 0).await.unwrap_err();
    match err {
        papers_zotero::ZoteroError::Api { status, .. } => assert_eq!(status, 412),
        e => panic!("Expected 412, got: {e}"),
    }
}

// ════════════════════════════════════════════════════════════════════
// create_searches — extra cases
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_create_searches_multi_condition() {
    let c = client();
    let resp = c
        .create_searches(vec![serde_json::json!({
            "name": "Multi-condition search",
            "conditions": [
                {"condition": "title", "operator": "contains", "value": "rust"},
                {"condition": "tag", "operator": "is", "value": "important"}
            ]
        })])
        .await
        .unwrap();
    assert!(resp.is_ok());
    let key = resp.successful_keys()[0].to_string();
    let _cleanup = SearchCleanup::new([key.clone()]);

    let search = c.get_search(&key).await.unwrap();
    assert_eq!(search.data.conditions.len(), 2);
}

// ════════════════════════════════════════════════════════════════════
// delete_searches — extra cases
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_delete_searches_version_conflict_returns_412() {
    let c = client();
    let resp = c
        .create_searches(vec![serde_json::json!({
            "name": "Search conflict test",
            "conditions": [{"condition": "tag", "operator": "is", "value": "x"}]
        })])
        .await
        .unwrap();
    assert!(resp.is_ok());
    let key = resp.successful_keys()[0].to_string();
    let _cleanup = SearchCleanup::new([key.clone()]);

    let err = c.delete_searches(&[key.clone()], 0).await.unwrap_err();
    match err {
        papers_zotero::ZoteroError::Api { status, .. } => assert_eq!(status, 412),
        e => panic!("Expected 412, got: {e}"),
    }
}

// ════════════════════════════════════════════════════════════════════
// delete_tags — extra cases
// ════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_delete_multiple_tags_at_once() {
    let c = client();
    let (key, _) = create_note(&c, "<p>multi tag delete</p>", &["multi-del-tag-a", "multi-del-tag-b"]).await;
    let _item = ItemCleanup::new([key.clone()]);

    let lv = library_version(&c).await;
    c.delete_tags(&["multi-del-tag-a".to_string(), "multi-del-tag-b".to_string()], lv)
        .await
        .unwrap();

    let tags = c.list_tags(&TagListParams::default()).await.unwrap();
    assert!(!tags.items.iter().any(|t| t.tag == "multi-del-tag-a"));
    assert!(!tags.items.iter().any(|t| t.tag == "multi-del-tag-b"));
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_delete_tags_version_conflict_returns_412() {
    let c = client();
    let (key, _) = create_note(&c, "<p>tag conflict</p>", &["tag-conflict-001"]).await;
    let _item = ItemCleanup::new([key.clone()]);
    let _tag = TagCleanup::new(["tag-conflict-001".to_string()]);

    let err = c.delete_tags(&["tag-conflict-001".to_string()], 0).await.unwrap_err();
    match err {
        papers_zotero::ZoteroError::Api { status, .. } => assert_eq!(status, 412),
        e => panic!("Expected 412, got: {e}"),
    }
}

// ════════════════════════════════════════════════════════════════════
// Full CRUD cycles (comprehensive happy-path per write method)
// ════════════════════════════════════════════════════════════════════

/// create_items + patch_item + update_item + delete_item full cycle.
#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_item_full_crud_cycle() {
    let c = client();
    let (key, _) = create_note(&c, "<p>CRUD cycle</p>", &["test-integration"]).await;
    let _cleanup = ItemCleanup::new([key.clone()]);

    let item = c.get_item(&key).await.unwrap();
    assert_eq!(item.data.note.as_deref(), Some("<p>CRUD cycle</p>"));
    let version = item.version;

    c.patch_item(&key, version, serde_json::json!({"note": "<p>Patched</p>"}))
        .await
        .unwrap();

    let item = c.get_item(&key).await.unwrap();
    assert_eq!(item.data.note.as_deref(), Some("<p>Patched</p>"));
    let version = item.version;

    c.update_item(
        &key,
        version,
        serde_json::json!({
            "key": key,
            "version": version,
            "itemType": "note",
            "note": "<p>PUT replaced</p>",
            "tags": [{"tag": "test-integration"}],
            "collections": [],
            "relations": {}
        }),
    )
    .await
    .unwrap();

    let item = c.get_item(&key).await.unwrap();
    assert_eq!(item.data.note.as_deref(), Some("<p>PUT replaced</p>"));
}

/// create_items multi-delete cycle.
#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_item_multi_delete_cycle() {
    let c = client();
    let resp = c
        .create_items(vec![
            serde_json::json!({"itemType": "note", "note": "<p>Multi A</p>", "tags": [], "collections": [], "relations": {}}),
            serde_json::json!({"itemType": "note", "note": "<p>Multi B</p>", "tags": [], "collections": [], "relations": {}}),
        ])
        .await
        .unwrap();
    assert!(resp.is_ok());
    let keys: Vec<String> = resp.successful_keys().into_iter().map(String::from).collect();
    let _cleanup = ItemCleanup::new(keys.clone());

    let lv = library_version(&c).await;
    c.delete_items(&keys, lv).await.unwrap();

    for key in &keys {
        match c.get_item(key).await {
            Err(papers_zotero::ZoteroError::Api { status, .. }) => assert_eq!(status, 404),
            Ok(_) => panic!("Item should be deleted"),
            Err(e) => panic!("Unexpected error: {e}"),
        }
    }
}

/// create_collections + update_collection + delete_collection full cycle.
#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_collection_full_crud_cycle() {
    let c = client();
    let (key, _) = create_collection(&c, "CRUD Collection").await;
    let _cleanup = CollectionCleanup::new([key.clone()]);

    let coll = c.get_collection(&key).await.unwrap();
    assert_eq!(coll.data.name, "CRUD Collection");
    let version = coll.version;

    c.update_collection(
        &key,
        version,
        serde_json::json!({
            "key": key,
            "version": version,
            "name": "Renamed",
            "parentCollection": false,
            "relations": {}
        }),
    )
    .await
    .unwrap();

    let coll = c.get_collection(&key).await.unwrap();
    assert_eq!(coll.data.name, "Renamed");
}

/// create_collections multi-delete cycle.
#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_collection_multi_delete_cycle() {
    let c = client();
    let resp = c
        .create_collections(vec![
            serde_json::json!({"name": "Multi Del A"}),
            serde_json::json!({"name": "Multi Del B"}),
        ])
        .await
        .unwrap();
    assert!(resp.is_ok());
    let keys: Vec<String> = resp.successful_keys().into_iter().map(String::from).collect();
    let _cleanup = CollectionCleanup::new(keys.clone());

    let lv = library_version(&c).await;
    c.delete_collections(&keys, lv).await.unwrap();

    for key in &keys {
        match c.get_collection(key).await {
            Err(papers_zotero::ZoteroError::Api { status, .. }) => assert_eq!(status, 404),
            Ok(_) => panic!("Collection should be deleted"),
            Err(e) => panic!("Unexpected error: {e}"),
        }
    }
}

/// create_searches + delete_searches full cycle.
#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_search_create_delete_cycle() {
    let c = client();
    let resp = c
        .create_searches(vec![serde_json::json!({
            "name": "CRUD Search",
            "conditions": [{"condition": "tag", "operator": "is", "value": "test-integration"}]
        })])
        .await
        .unwrap();
    assert!(resp.is_ok());
    let key = resp.successful_keys()[0].to_string();
    let _cleanup = SearchCleanup::new([key.clone()]);

    let searches = c.list_searches().await.unwrap();
    assert!(searches.items.iter().any(|s| s.key == key));

    let lv = library_version(&c).await;
    c.delete_searches(&[key.clone()], lv).await.unwrap();

    let after = c.list_searches().await.unwrap();
    assert!(!after.items.iter().any(|s| s.key == key));
}

/// delete_tags full cycle: create tagged item, delete tag, verify gone.
#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_tag_delete_cycle() {
    let c = client();
    let (key, _) = create_note(&c, "<p>tag cycle</p>", &["tag-cycle-001"]).await;
    let _item = ItemCleanup::new([key.clone()]);
    let _tag = TagCleanup::new(["tag-cycle-001".to_string()]);

    let tags = c.list_tags(&TagListParams::default()).await.unwrap();
    assert!(tags.items.iter().any(|t| t.tag == "tag-cycle-001"));

    let lv = library_version(&c).await;
    c.delete_tags(&["tag-cycle-001".to_string()], lv).await.unwrap();

    let after = c.list_tags(&TagListParams::default()).await.unwrap();
    assert!(!after.items.iter().any(|t| t.tag == "tag-cycle-001"));
}
