/// Integration tests for DataLab caching in `do_extract`.
///
/// PDFs are downloaded lazily into `tests/.data/` (git-ignored) so repeated
/// runs don't re-download. Tests are skipped automatically if `DATALAB_API_KEY`
/// is not set.
use papers_core::text::{do_extract, PdfSource, ProcessingMode};
use papers_datalab::DatalabClient;
use std::path::PathBuf;

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

#[tokio::test]
async fn test_datalab_cache_miss_then_hit() {
    let datalab_key = match std::env::var("DATALAB_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            eprintln!("[test] DATALAB_API_KEY not set — skipping");
            return;
        }
    };

    let pdf_bytes = match get_pdf().await {
        Some(b) => b,
        None => {
            eprintln!("[test] ZOTERO_API_KEY not set — skipping");
            return;
        }
    };

    // Clear any existing cache so we get a clean miss.
    let cdir = cache_dir();
    let _ = std::fs::remove_dir_all(&cdir);

    let dl = DatalabClient::new(datalab_key);
    let mode = ProcessingMode::Balanced;

    // --- First call: cache miss, hits DataLab API ---
    let mut source = PdfSource::ZoteroLocal { path: "test".into() };
    let t0 = std::time::Instant::now();
    let text1: String = do_extract(pdf_bytes.clone(), ZOTERO_ITEM_KEY, Some((&dl, mode.clone())), &mut source)
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
    eprintln!("[test] API call took {:?}", api_duration);

    // --- Second call: cache hit, should return immediately ---
    let mut source2 = PdfSource::ZoteroLocal { path: "test".into() };
    let t1 = std::time::Instant::now();
    let text2: String = do_extract(pdf_bytes.clone(), ZOTERO_ITEM_KEY, Some((&dl, mode)), &mut source2)
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
