use papers_core::zotero::{resolve_collection_key, resolve_item_key, resolve_search_key};
use papers_zotero::ZoteroClient;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn make_client(mock: &MockServer) -> ZoteroClient {
    ZoteroClient::new("test", "test-key").with_base_url(mock.uri())
}

fn array_response(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("Total-Results", "1")
        .insert_header("Last-Modified-Version", "100")
        .set_body_string(body)
}

fn item_list_json() -> &'static str {
    r#"[{
        "key": "ABC12345",
        "version": 1,
        "library": {"type": "user", "id": 1, "name": "test", "links": {}},
        "links": {},
        "meta": {},
        "data": {
            "key": "ABC12345",
            "version": 1,
            "itemType": "journalArticle",
            "title": "Attention Is All You Need",
            "creators": [],
            "tags": [],
            "collections": [],
            "relations": {},
            "dateAdded": "2024-01-01T00:00:00Z",
            "dateModified": "2024-01-01T00:00:00Z"
        }
    }]"#
}

fn collection_list_json() -> &'static str {
    r#"[{
        "key": "COL12345",
        "version": 1,
        "library": {"type": "user", "id": 1, "name": "test", "links": {}},
        "links": {},
        "meta": {"numCollections": 0, "numItems": 5},
        "data": {
            "key": "COL12345",
            "version": 1,
            "name": "GPU Papers",
            "parentCollection": false,
            "relations": {}
        }
    }]"#
}

fn search_list_json() -> &'static str {
    r#"[{
        "key": "SCH12345",
        "version": 1,
        "library": {"type": "user", "id": 1, "name": "test", "links": {}},
        "links": {},
        "data": {
            "key": "SCH12345",
            "version": 1,
            "name": "Recent ML papers",
            "conditions": []
        }
    }]"#
}

// ── resolve_item_key ──────────────────────────────────────────────────

#[tokio::test]
async fn test_resolve_item_key_direct_key_passthrough() {
    // When input looks like a key, no HTTP call should be made.
    let mock = MockServer::start().await;
    let client = make_client(&mock);
    let result = resolve_item_key(&client, "ABC12345").await.unwrap();
    assert_eq!(result, "ABC12345");
    // Verify no requests were made to the mock server
    assert!(mock.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn test_resolve_item_key_by_title() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/test/items/top"))
        .and(query_param("q", "Attention Is All You Need"))
        .and(query_param("limit", "1"))
        .respond_with(array_response(item_list_json()))
        .mount(&mock)
        .await;

    let client = make_client(&mock);
    let result = resolve_item_key(&client, "Attention Is All You Need").await.unwrap();
    assert_eq!(result, "ABC12345");
}

#[tokio::test]
async fn test_resolve_item_key_not_found() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/test/items/top"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Total-Results", "0")
                .insert_header("Last-Modified-Version", "1")
                .set_body_string("[]"),
        )
        .mount(&mock)
        .await;

    let client = make_client(&mock);
    let result = resolve_item_key(&client, "nonexistent paper title xyz").await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("nonexistent paper title xyz"));
}

// ── resolve_collection_key ────────────────────────────────────────────

#[tokio::test]
async fn test_resolve_collection_key_direct_key_passthrough() {
    let mock = MockServer::start().await;
    let client = make_client(&mock);
    let result = resolve_collection_key(&client, "COL12345").await.unwrap();
    assert_eq!(result, "COL12345");
    assert!(mock.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn test_resolve_collection_key_by_name() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/test/collections"))
        .and(query_param("limit", "100"))
        .respond_with(array_response(collection_list_json()))
        .mount(&mock)
        .await;

    let client = make_client(&mock);
    let result = resolve_collection_key(&client, "GPU Papers").await.unwrap();
    assert_eq!(result, "COL12345");
}

#[tokio::test]
async fn test_resolve_collection_key_by_name_case_insensitive() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/test/collections"))
        .and(query_param("limit", "100"))
        .respond_with(array_response(collection_list_json()))
        .mount(&mock)
        .await;

    let client = make_client(&mock);
    // Lowercase should still match "GPU Papers"
    let result = resolve_collection_key(&client, "gpu papers").await.unwrap();
    assert_eq!(result, "COL12345");
}

#[tokio::test]
async fn test_resolve_collection_key_by_partial_name() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/test/collections"))
        .and(query_param("limit", "100"))
        .respond_with(array_response(collection_list_json()))
        .mount(&mock)
        .await;

    let client = make_client(&mock);
    // Partial match should work
    let result = resolve_collection_key(&client, "GPU").await.unwrap();
    assert_eq!(result, "COL12345");
}

#[tokio::test]
async fn test_resolve_collection_key_not_found() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/test/collections"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Total-Results", "0")
                .insert_header("Last-Modified-Version", "1")
                .set_body_string("[]"),
        )
        .mount(&mock)
        .await;

    let client = make_client(&mock);
    let result = resolve_collection_key(&client, "nonexistent collection").await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("nonexistent collection"));
}

// ── resolve_search_key ────────────────────────────────────────────────

#[tokio::test]
async fn test_resolve_search_key_direct_key_passthrough() {
    let mock = MockServer::start().await;
    let client = make_client(&mock);
    let result = resolve_search_key(&client, "SCH12345").await.unwrap();
    assert_eq!(result, "SCH12345");
    assert!(mock.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn test_resolve_search_key_by_name() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/test/searches"))
        .respond_with(array_response(search_list_json()))
        .mount(&mock)
        .await;

    let client = make_client(&mock);
    let result = resolve_search_key(&client, "Recent ML papers").await.unwrap();
    assert_eq!(result, "SCH12345");
}

#[tokio::test]
async fn test_resolve_search_key_by_partial_name() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/test/searches"))
        .respond_with(array_response(search_list_json()))
        .mount(&mock)
        .await;

    let client = make_client(&mock);
    let result = resolve_search_key(&client, "ML").await.unwrap();
    assert_eq!(result, "SCH12345");
}

#[tokio::test]
async fn test_resolve_search_key_not_found() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/test/searches"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Total-Results", "0")
                .insert_header("Last-Modified-Version", "1")
                .set_body_string("[]"),
        )
        .mount(&mock)
        .await;

    let client = make_client(&mock);
    let result = resolve_search_key(&client, "nonexistent search").await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("nonexistent search"));
}
