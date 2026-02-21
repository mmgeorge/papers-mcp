use papers_zotero::{
    CollectionListParams, DeletedParams, FulltextParams, ItemListParams, TagListParams, ZoteroClient,
};

fn client() -> ZoteroClient {
    let user_id =
        std::env::var("ZOTERO_TEST_USER_ID").expect("ZOTERO_TEST_USER_ID must be set for live tests");
    let api_key =
        std::env::var("ZOTERO_TEST_API_KEY").expect("ZOTERO_TEST_API_KEY must be set for live tests");
    ZoteroClient::new(user_id, api_key)
}

// ── Live item tests ──────────────────────────────────────────────────

#[tokio::test]
async fn test_live_list_items() {
    let params = ItemListParams::builder().limit(1).build();
    let resp = client().list_items(&params).await.unwrap();
    assert!(resp.total_results.unwrap_or(0) > 0);
    assert_eq!(resp.items.len(), 1);
}

#[tokio::test]
async fn test_live_list_top_items() {
    let params = ItemListParams::builder().limit(1).build();
    let resp = client().list_top_items(&params).await.unwrap();
    assert!(resp.total_results.unwrap_or(0) > 0);
    assert_eq!(resp.items.len(), 1);
}

#[tokio::test]
async fn test_live_list_trash_items() {
    let params = ItemListParams::builder().limit(1).build();
    let resp = client().list_trash_items(&params).await.unwrap();
    // Trash may be empty, that's OK
    assert!(resp.total_results.is_some());
}

#[tokio::test]
async fn test_live_get_item() {
    // First list to get a key, then get that item
    let params = ItemListParams::builder().limit(1).build();
    let list_resp = client().list_items(&params).await.unwrap();
    assert!(!list_resp.items.is_empty());
    let key = &list_resp.items[0].key;
    let item = client().get_item(key).await.unwrap();
    assert_eq!(&item.key, key);
}

#[tokio::test]
async fn test_live_list_item_children() {
    // Find an item that has children
    let params = ItemListParams::builder()
        .item_type("-attachment || note")
        .limit(5)
        .build();
    let list_resp = client().list_items(&params).await.unwrap();
    if let Some(parent) = list_resp
        .items
        .iter()
        .find(|i| i.meta.num_children.unwrap_or(0) > 0)
    {
        let children = client()
            .list_item_children(&parent.key, &ItemListParams::default())
            .await
            .unwrap();
        assert!(!children.items.is_empty());
    }
}

#[tokio::test]
async fn test_live_search_items() {
    let params = ItemListParams::builder().q("rendering").limit(5).build();
    let resp = client().list_items(&params).await.unwrap();
    assert!(!resp.items.is_empty());
}

#[tokio::test]
async fn test_live_search_xz_ordering() {
    let zotero = client();

    // Test DOI-based search (what try_zotero actually uses)
    let doi = "10.1007/3-540-48482-5_7";
    println!("\n=== DOI search: {:?} ===", doi);
    let doi_params = ItemListParams::builder().q(doi).qmode("everything").limit(5).build();
    let doi_resp = zotero.list_items(&doi_params).await.unwrap();
    println!("DOI search results: {:?}", doi_resp.total_results);
    for item in &doi_resp.items {
        println!("  key={} doi={:?} title={:?}", item.key, item.data.doi, item.data.title);
    }

    for query in &["XZ ordering", "XZ-Ordering", "GeoMesa"] {
        println!("\n=== Search: {:?} ===", query);
        let params = ItemListParams::builder().q(*query).limit(10).build();
        let resp = zotero.list_items(&params).await.unwrap();
        println!("Total results: {:?}", resp.total_results);

        for item in &resp.items {
            let title = item.data.title.as_deref().unwrap_or("<no title>");
            let doi = item.data.doi.as_deref().unwrap_or("<no doi>");
            println!("  key={} doi={:?} title={:?}", item.key, doi, title);

            // Check for PDF children
            let children = zotero
                .list_item_children(&item.key, &ItemListParams::default())
                .await
                .unwrap();
            for child in &children.items {
                let content_type = child.data.content_type.as_deref().unwrap_or("");
                let link_mode = child.data.link_mode.as_deref().unwrap_or("");
                let filename = child.data.filename.as_deref().unwrap_or("");
                println!(
                    "    child={} content_type={:?} link_mode={:?} filename={:?}",
                    child.key, content_type, link_mode, filename
                );
                // Check local file path
                if !filename.is_empty() {
                    let home = dirs::home_dir().unwrap_or_default();
                    let local = home.join("Zotero").join("storage").join(&child.key).join(filename);
                    println!("    local_path={} exists={}", local.display(), local.exists());
                }
            }
        }
    }
}

// ── Live collection tests ────────────────────────────────────────────

#[tokio::test]
async fn test_live_list_collections() {
    let params = CollectionListParams::builder().limit(5).build();
    let resp = client().list_collections(&params).await.unwrap();
    assert!(resp.total_results.unwrap_or(0) > 0);
    assert!(!resp.items.is_empty());
}

#[tokio::test]
async fn test_live_list_top_collections() {
    let resp = client()
        .list_top_collections(&CollectionListParams::default())
        .await
        .unwrap();
    assert!(!resp.items.is_empty());
}

#[tokio::test]
async fn test_live_get_collection() {
    let list_resp = client()
        .list_collections(&CollectionListParams::builder().limit(1).build())
        .await
        .unwrap();
    assert!(!list_resp.items.is_empty());
    let key = &list_resp.items[0].key;
    let coll = client().get_collection(key).await.unwrap();
    assert_eq!(&coll.key, key);
}

#[tokio::test]
async fn test_live_list_collection_items() {
    let list_resp = client()
        .list_collections(&CollectionListParams::builder().limit(1).build())
        .await
        .unwrap();
    if let Some(coll) = list_resp.items.first() {
        let items = client()
            .list_collection_items(&coll.key, &ItemListParams::builder().limit(1).build())
            .await
            .unwrap();
        assert!(items.total_results.is_some());
    }
}

// ── Live tag tests ───────────────────────────────────────────────────

#[tokio::test]
async fn test_live_list_tags() {
    let params = TagListParams::builder().limit(5).build();
    let resp = client().list_tags(&params).await.unwrap();
    // Note: Total-Results may be 0 for tags even when items are returned
    assert!(resp.total_results.is_some());
    assert!(!resp.items.is_empty());
}

#[tokio::test]
async fn test_live_list_items_tags() {
    let resp = client()
        .list_items_tags(&TagListParams::builder().limit(5).build())
        .await
        .unwrap();
    assert!(!resp.items.is_empty());
}

// ── Live search tests ────────────────────────────────────────────────

#[tokio::test]
async fn test_live_list_searches() {
    let resp = client().list_searches().await.unwrap();
    // May be empty if no saved searches
    assert!(resp.total_results.is_some());
}

// ── Live group tests ─────────────────────────────────────────────────

#[tokio::test]
async fn test_live_list_groups() {
    let resp = client().list_groups().await.unwrap();
    // May be empty if user has no groups
    assert!(resp.total_results.is_some());
}

// ── Live key info test ───────────────────────────────────────────────

#[tokio::test]
async fn test_live_key_info() {
    let info = client().get_key_info().await.unwrap();
    assert!(info.get("userID").is_some() || info.get("key").is_some());
}

#[tokio::test]
async fn test_live_current_key_info() {
    let info = client().get_current_key_info().await.unwrap();
    assert!(info.get("userID").is_some());
    assert!(info.get("username").is_some());
    assert!(info.get("access").is_some());
}

// ── Live full-text tests ─────────────────────────────────────────────

#[tokio::test]
async fn test_live_list_fulltext_versions() {
    let resp = client()
        .list_fulltext_versions(&FulltextParams::default())
        .await
        .unwrap();
    // Most libraries have at least some indexed PDFs
    assert!(!resp.data.is_empty(), "expected some fulltext entries");
    assert!(resp.last_modified_version.is_some());
    // All values should be positive version numbers
    for (key, version) in &resp.data {
        assert!(!key.is_empty());
        assert!(*version > 0, "version for {key} should be > 0");
    }
}

#[tokio::test]
async fn test_live_list_fulltext_versions_since() {
    // First get the current library version
    let all = client()
        .list_fulltext_versions(&FulltextParams::default())
        .await
        .unwrap();
    let current_version = all.last_modified_version.unwrap_or(0);
    // Fetch with since = current version — should be empty (no new content)
    let resp = client()
        .list_fulltext_versions(&FulltextParams::builder().since(current_version).build())
        .await
        .unwrap();
    assert!(resp.data.is_empty(), "no new fulltext since current version");
}

#[tokio::test]
async fn test_live_get_item_fulltext() {
    // Find an attachment key from the fulltext index
    let versions = client()
        .list_fulltext_versions(&FulltextParams::default())
        .await
        .unwrap();
    let (key, _) = versions.data.iter().next().expect("need at least one fulltext entry");
    let resp = client().get_item_fulltext(key).await.unwrap();
    assert!(!resp.data.content.is_empty());
    assert!(resp.last_modified_version.is_some());
    // Should have either page counts or char counts
    let has_pages = resp.data.indexed_pages.is_some();
    let has_chars = resp.data.indexed_chars.is_some();
    assert!(has_pages || has_chars, "expected page or char count");
}

// ── Live deleted test ────────────────────────────────────────────────

#[tokio::test]
async fn test_live_get_deleted() {
    let params = DeletedParams::builder().since(0u64).build();
    let resp = client().get_deleted(&params).await.unwrap();
    assert!(resp.last_modified_version.is_some());
    // Fields exist (may be empty arrays if nothing was deleted)
    let _ = resp.data.collections.len();
    let _ = resp.data.items.len();
    let _ = resp.data.searches.len();
    let _ = resp.data.tags.len();
    let _ = resp.data.settings.len();
}

// ── Live settings tests ──────────────────────────────────────────────

#[tokio::test]
async fn test_live_get_settings() {
    let resp = client().get_settings().await.unwrap();
    assert!(resp.last_modified_version.is_some());
    // tagColors should be present in most libraries
    if let Some(tc) = resp.data.get("tagColors") {
        assert!(tc.value.is_array());
        assert!(tc.version > 0);
    }
}

#[tokio::test]
async fn test_live_get_setting_tag_colors() {
    let resp = client().get_setting("tagColors").await.unwrap();
    assert!(resp.last_modified_version.is_some());
    assert!(resp.data.value.is_array());
    assert!(resp.data.version > 0);
    // Each entry should have name and color
    for entry in resp.data.value.as_array().unwrap() {
        assert!(entry.get("name").is_some());
        assert!(entry.get("color").is_some());
    }
}

// ── Live file view tests ─────────────────────────────────────────────

#[tokio::test]
async fn test_live_get_item_file_view_url() {
    // Find an imported_file attachment
    let versions = client()
        .list_fulltext_versions(&FulltextParams::default())
        .await
        .unwrap();
    let (key, _) = versions.data.iter().next().expect("need at least one attachment");
    let url = client().get_item_file_view_url(key).await.unwrap();
    assert!(url.starts_with("https://"), "expected https URL, got: {url}");
}

#[tokio::test]
async fn test_live_get_item_file_view() {
    let versions = client()
        .list_fulltext_versions(&FulltextParams::default())
        .await
        .unwrap();
    let (key, _) = versions.data.iter().next().expect("need at least one attachment");
    let bytes = client().get_item_file_view(key).await.unwrap();
    assert!(!bytes.is_empty());
    // PDFs start with %PDF
    if bytes.starts_with(b"%PDF") {
        println!("Got PDF: {} bytes", bytes.len());
    }
}
