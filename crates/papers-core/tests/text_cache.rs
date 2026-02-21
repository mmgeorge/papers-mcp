/// Integration tests for DataLab caching in `do_extract`.
///
/// PDFs are downloaded lazily into `tests/.data/` (git-ignored) so repeated
/// runs don't re-download.
///
/// **DataLab is always mocked with wiremock** — never call the real DataLab API
/// from tests. Real calls spend credits and take ~25 seconds.
use papers_core::text::{do_extract, PdfSource, ProcessingMode};
use papers_datalab::DatalabClient;
use papers_zotero::{ItemListParams, ZoteroClient};
use std::path::PathBuf;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Zotero parent item key for "Augmented Vertex Block Descent" (Giles et al.)
const ZOTERO_ITEM_KEY: &str = "U9PRIZJ7";
/// Zotero attachment key for the PDF of the above item
const ZOTERO_ATTACHMENT_KEY: &str = "QXNY8AX8";

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(".data")
}

/// Download the AVBD PDF from Zotero into `.data/`, or return it from disk if
/// already cached. Skips (returns `None`) if `ZOTERO_API_KEY` is not set.
async fn get_pdf() -> Option<Vec<u8>> {
    let path = data_dir().join(format!("{}.pdf", ZOTERO_ITEM_KEY));
    if path.exists() {
        return Some(std::fs::read(&path).expect("failed to read cached PDF"));
    }

    let api_key = std::env::var("ZOTERO_API_KEY").ok()?;
    let user_id = std::env::var("ZOTERO_USER_ID")
        .unwrap_or_else(|_| "16916553".to_string());

    let zotero = papers_zotero::ZoteroClient::new(&user_id, &api_key);
    let bytes = zotero
        .download_item_file(ZOTERO_ATTACHMENT_KEY)
        .await
        .expect("failed to download PDF from Zotero");

    assert!(!bytes.is_empty(), "downloaded PDF was empty");

    std::fs::create_dir_all(data_dir()).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    eprintln!("[test] downloaded PDF to {}", path.display());

    Some(bytes)
}

fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .expect("no cache dir")
        .join("papers")
        .join("datalab")
        .join(ZOTERO_ITEM_KEY)
}

/// Spin up a wiremock DataLab mock and return a DatalabClient pointed at it.
///
/// Mocks two endpoints:
/// - `POST /api/v1/marker` → submit response with `request_id = "test-req-1"`
/// - `GET  /api/v1/marker/test-req-1` → completed response with markdown + json
///
/// The mock server is kept alive as long as the returned `MockServer` is in scope.
async fn setup_datalab_mock() -> (MockServer, DatalabClient) {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/marker"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "request_id": "test-req-1",
            "request_check_url": "http://mock/api/v1/marker/test-req-1",
            "success": true
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(r"^/api/v1/marker/test-req-1$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "complete",
            "success": true,
            "markdown": "# Test Paper\n\nExtracted content for testing.",
            "json": {"pages": [{"blocks": []}]}
        })))
        .mount(&server)
        .await;

    let client = DatalabClient::new("mock-key").with_base_url(server.uri());
    (server, client)
}

/// Write a minimal fake local DataLab cache for `id`. Returns the cache dir path.
fn write_fake_local_cache(id: &str) -> PathBuf {
    let dir = dirs::cache_dir()
        .expect("no cache dir")
        .join("papers")
        .join("datalab")
        .join(id);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{id}.md")), "# Fake cached paper\n\nContent.").unwrap();
    std::fs::write(dir.join(format!("{id}.json")), r#"{"pages":[]}"#).unwrap();
    dir
}

/// Build a test ZoteroClient and create a temporary journalArticle in the test
/// library. Returns `(client, parent_key)`. Skips (returns `None`) if test
/// credentials are not set.
async fn make_test_zotero_item() -> Option<(ZoteroClient, String)> {
    let (user_id, api_key) = match (
        std::env::var("ZOTERO_TEST_USER_ID"),
        std::env::var("ZOTERO_TEST_API_KEY"),
    ) {
        (Ok(u), Ok(k)) if !u.is_empty() && !k.is_empty() => (u, k),
        _ => return None,
    };
    let zotero = ZoteroClient::new(user_id, api_key);
    let resp = zotero
        .create_items(vec![serde_json::json!({
            "itemType": "journalArticle",
            "title": "papers-core test — safe to delete",
            "tags": [],
            "collections": []
        })])
        .await
        .expect("failed to create temp parent item");
    let key = resp.successful_keys().into_iter().next()
        .expect("create_items returned no key");
    Some((zotero, key))
}

/// Count how many `Papers.zip` imported_file attachments are on `parent_key`.
async fn count_papers_zip(zotero: &ZoteroClient, parent_key: &str) -> usize {
    zotero
        .list_item_children(parent_key, &ItemListParams::default())
        .await
        .map(|r| {
            r.items
                .iter()
                .filter(|c| c.data.filename.as_deref() == Some("Papers.zip"))
                .count()
        })
        .unwrap_or(0)
}

/// Poll until `Papers.zip` appears under `parent_key` or `timeout_secs` elapses.
async fn wait_for_papers_zip(
    zotero: &ZoteroClient,
    parent_key: &str,
    timeout_secs: u64,
) -> bool {
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    while std::time::Instant::now() < deadline {
        if count_papers_zip(zotero, parent_key).await > 0 {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    false
}

/// Drop guard: deletes a Zotero item (and all its children) on drop.
struct ZoteroItemCleanup(ZoteroClient, String);
impl Drop for ZoteroItemCleanup {
    fn drop(&mut self) {
        let zotero = self.0.clone();
        let key = self.1.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                if let Ok(item) = zotero.get_item(&key).await {
                    let _ = zotero.delete_item(&key, item.version).await;
                }
            })
        });
    }
}

#[tokio::test]
async fn test_datalab_cache_miss_then_hit() {
    let pdf_bytes = match get_pdf().await {
        Some(b) => b,
        None => {
            eprintln!("[test] ZOTERO_API_KEY not set — skipping PDF download");
            // Use a dummy single-byte payload so we can still test the cache layer.
            vec![0u8]
        }
    };

    // Clear any existing cache so we get a clean miss.
    let cdir = cache_dir();
    let _ = std::fs::remove_dir_all(&cdir);

    let (_server, dl) = setup_datalab_mock().await;
    let mode = ProcessingMode::Balanced;

    // --- First call: cache miss, hits (mocked) DataLab ---
    let mut source = PdfSource::ZoteroLocal { path: "test".into() };
    let t0 = std::time::Instant::now();
    let text1: String = do_extract(pdf_bytes.clone(), ZOTERO_ITEM_KEY, None, Some((&dl, mode.clone())), &mut source)
        .await
        .expect("do_extract failed on first call");
    let api_duration = t0.elapsed();

    assert!(!text1.is_empty(), "first call returned empty text");
    assert!(
        matches!(source, PdfSource::DataLab),
        "source should be DataLab after extraction"
    );

    // Verify cache files were written.
    let md_path = cdir.join(format!("{}.md", ZOTERO_ITEM_KEY));
    let json_path = cdir.join(format!("{}.json", ZOTERO_ITEM_KEY));
    assert!(md_path.exists(), ".md cache file missing");
    assert!(json_path.exists(), ".json cache file missing");
    eprintln!("[test] first call (mocked) took {:?}", api_duration);

    // --- Second call: cache hit, should return immediately without hitting mock ---
    let mut source2 = PdfSource::ZoteroLocal { path: "test".into() };
    let t1 = std::time::Instant::now();
    let text2: String = do_extract(pdf_bytes.clone(), ZOTERO_ITEM_KEY, None, Some((&dl, mode)), &mut source2)
        .await
        .expect("do_extract failed on second call");
    let cache_duration = t1.elapsed();

    assert_eq!(text1, text2, "cached text should match API text");
    assert!(
        cache_duration < std::time::Duration::from_millis(500),
        "cache hit took too long: {:?}",
        cache_duration
    );
    eprintln!("[test] Cache hit took {:?}", cache_duration);
}

/// Full two-way Zotero sync round-trip:
///   1. Mocked DataLab → local cache + Papers.zip uploaded to Zotero
///   2. Delete local cache
///   3. Restore from Zotero Papers.zip (no DataLab call)
///   4. Texts match
///
/// Requires: ZOTERO_TEST_USER_ID and ZOTERO_TEST_API_KEY (dedicated test library
/// with write access). DataLab is mocked — no DATALAB_API_KEY needed.
///
/// Uses `flavor = "multi_thread"` because the drop guard calls `block_in_place`.
#[tokio::test(flavor = "multi_thread")]
async fn test_zotero_sync_round_trip() {
    let (zotero, parent_key) = match make_test_zotero_item().await {
        Some(v) => v,
        None => {
            eprintln!("[test] ZOTERO_TEST_* not set — skipping");
            return;
        }
    };
    let _cleanup = ZoteroItemCleanup(zotero.clone(), parent_key.clone());
    eprintln!("[test] created temp parent item {parent_key}");

    // PDF bytes — fall back to a minimal dummy if the main-library key isn't set.
    let pdf_bytes = get_pdf().await.unwrap_or_else(|| vec![0u8]);
    let (_server, dl) = setup_datalab_mock().await;
    let mode = ProcessingMode::Fast;

    let cdir = dirs::cache_dir()
        .expect("no cache dir")
        .join("papers")
        .join("datalab")
        .join(&parent_key);
    let _ = std::fs::remove_dir_all(&cdir);

    // --- First call: local miss + Zotero miss → mocked DataLab → upload Papers.zip ---
    let mut source = PdfSource::ZoteroLocal { path: "test".into() };
    let t0 = std::time::Instant::now();
    let text1 = do_extract(
        pdf_bytes.clone(),
        &parent_key,
        Some(&zotero),
        Some((&dl, mode.clone())),
        &mut source,
    )
    .await
    .expect("first do_extract failed");
    eprintln!("[test] first call (mocked DataLab + Zotero upload) took {:?}", t0.elapsed());

    assert!(!text1.is_empty(), "first call returned empty text");
    assert!(matches!(source, PdfSource::DataLab));
    assert!(cdir.join(format!("{parent_key}.md")).exists(), "local .md not written");
    assert_eq!(count_papers_zip(&zotero, &parent_key).await, 1, "Papers.zip not uploaded");

    // --- Delete local cache to force Zotero restore path ---
    std::fs::remove_dir_all(&cdir).expect("failed to remove cache dir");
    assert!(!cdir.exists());

    // --- Second call: local miss → Zotero Papers.zip hit → no DataLab call ---
    let mut source2 = PdfSource::ZoteroLocal { path: "test".into() };
    let t1 = std::time::Instant::now();
    let text2 = do_extract(
        pdf_bytes.clone(),
        &parent_key,
        Some(&zotero),
        Some((&dl, mode)),
        &mut source2,
    )
    .await
    .expect("second do_extract failed");
    let restore_dur = t1.elapsed();
    eprintln!("[test] second call (Zotero restore) took {:?}", restore_dur);

    assert_eq!(text1, text2, "restored text must match original");
    assert!(matches!(source2, PdfSource::DataLab));
    assert!(
        restore_dur < std::time::Duration::from_secs(30),
        "Zotero restore took unexpectedly long: {restore_dur:?}"
    );

    let _ = std::fs::remove_dir_all(&cdir);
}

/// Local cache exists but no Papers.zip in Zotero yet.
///
/// `do_extract` should return from the local cache immediately, then the
/// background task should upload a new Papers.zip attachment.
///
/// Requires ZOTERO_TEST_USER_ID / ZOTERO_TEST_API_KEY.
#[tokio::test(flavor = "multi_thread")]
async fn test_local_cache_hit_uploads_papers_zip() {
    let (zotero, parent_key) = match make_test_zotero_item().await {
        Some(v) => v,
        None => {
            eprintln!("[test] ZOTERO_TEST_* not set — skipping");
            return;
        }
    };
    let _cleanup = ZoteroItemCleanup(zotero.clone(), parent_key.clone());

    let cdir = write_fake_local_cache(&parent_key);
    let (_server, dl) = setup_datalab_mock().await;
    let mode = ProcessingMode::Fast;

    // No Papers.zip yet
    assert_eq!(count_papers_zip(&zotero, &parent_key).await, 0);

    // Call do_extract — local cache hit, background upload spawned
    let mut source = PdfSource::ZoteroLocal { path: "test".into() };
    let t0 = std::time::Instant::now();
    let text = do_extract(vec![0u8], &parent_key, Some(&zotero), Some((&dl, mode)), &mut source)
        .await
        .expect("do_extract failed");
    let elapsed = t0.elapsed();

    assert!(!text.is_empty());
    assert!(matches!(source, PdfSource::DataLab));
    // Should return from local cache immediately — no DataLab call
    assert!(elapsed < std::time::Duration::from_millis(500), "cache hit took too long: {elapsed:?}");

    // Background task should upload Papers.zip — poll until it appears
    let uploaded = wait_for_papers_zip(&zotero, &parent_key, 15).await;
    assert!(uploaded, "Papers.zip not uploaded within 15s");
    assert_eq!(count_papers_zip(&zotero, &parent_key).await, 1);

    let _ = std::fs::remove_dir_all(&cdir);
}

/// Both local cache and Papers.zip in Zotero already exist.
///
/// `do_extract` should return from the local cache immediately. The background
/// task should find the existing Papers.zip and skip the upload — so the count
/// must stay at exactly 1 (no duplicate attachment created).
///
/// Requires ZOTERO_TEST_USER_ID / ZOTERO_TEST_API_KEY.
#[tokio::test(flavor = "multi_thread")]
async fn test_local_cache_and_papers_zip_both_exist() {
    let (zotero, parent_key) = match make_test_zotero_item().await {
        Some(v) => v,
        None => {
            eprintln!("[test] ZOTERO_TEST_* not set — skipping");
            return;
        }
    };
    let _cleanup = ZoteroItemCleanup(zotero.clone(), parent_key.clone());

    let cdir = write_fake_local_cache(&parent_key);
    let (_server, dl) = setup_datalab_mock().await;
    let mode = ProcessingMode::Fast;

    // First call: local cache hit → background uploads Papers.zip
    let mut source = PdfSource::ZoteroLocal { path: "test".into() };
    do_extract(vec![0u8], &parent_key, Some(&zotero), Some((&dl, mode.clone())), &mut source)
        .await
        .expect("first do_extract failed");

    // Wait for the upload so we start the second call with Papers.zip present
    let uploaded = wait_for_papers_zip(&zotero, &parent_key, 15).await;
    assert!(uploaded, "Papers.zip not uploaded within 15s after first call");
    assert_eq!(count_papers_zip(&zotero, &parent_key).await, 1);

    // Second call: both local cache and Papers.zip present
    let mut source2 = PdfSource::ZoteroLocal { path: "test".into() };
    let t0 = std::time::Instant::now();
    let text = do_extract(vec![0u8], &parent_key, Some(&zotero), Some((&dl, mode)), &mut source2)
        .await
        .expect("second do_extract failed");
    let elapsed = t0.elapsed();

    assert!(!text.is_empty());
    assert!(matches!(source2, PdfSource::DataLab));
    assert!(elapsed < std::time::Duration::from_millis(500), "cache hit took too long: {elapsed:?}");

    // Give the background task time to run, then verify no duplicate was created
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    assert_eq!(
        count_papers_zip(&zotero, &parent_key).await,
        1,
        "Papers.zip was duplicated — background task should have skipped upload"
    );

    let _ = std::fs::remove_dir_all(&cdir);
}

/// Verify the cached markdown for U9PRIZJ7 starts with expected paper content.
/// This test uses the on-disk cache written by `test_datalab_cache_miss_then_hit`
/// and does not make any network calls.
#[tokio::test]
async fn test_cached_markdown_content() {
    let md_path = cache_dir().join(format!("{}.md", ZOTERO_ITEM_KEY));
    if !md_path.exists() {
        eprintln!("[test] cache not populated yet — run test_datalab_cache_miss_then_hit first");
        return;
    }

    let markdown = std::fs::read_to_string(&md_path).expect("failed to read cached markdown");
    let first_lines: Vec<&str> = markdown.lines().take(10).collect();

    eprintln!("[test] First 10 lines of cached markdown:");
    for line in &first_lines {
        eprintln!("  {line}");
    }

    assert!(!markdown.is_empty(), "cached markdown is empty");
    // The mocked markdown always starts with "# Test Paper"
    let header = first_lines.join(" ").to_lowercase();
    assert!(
        header.contains("test") || header.contains("augmented") || header.contains("vertex"),
        "unexpected first lines: {header:?}"
    );
}
