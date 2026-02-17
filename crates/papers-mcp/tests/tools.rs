use openalex::OpenAlexClient;
use papers_mcp::server::PapersMcp;
use rmcp::handler::server::wrapper::Parameters;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn minimal_list_json() -> String {
    r#"{
        "meta": {"count": 42, "db_response_time_ms": 10, "page": 1, "per_page": 25, "next_cursor": null, "groups_count": null},
        "results": [],
        "group_by": []
    }"#
    .to_string()
}

fn minimal_work_json() -> String {
    r#"{
        "id": "https://openalex.org/W2741809807",
        "doi": "https://doi.org/10.7717/peerj.4375",
        "title": "The state of OA",
        "display_name": "The state of OA",
        "publication_year": 2018,
        "type": "article",
        "cited_by_count": 1234
    }"#
    .to_string()
}

fn minimal_autocomplete_json() -> String {
    r#"{
        "meta": {"count": 1, "db_response_time_ms": 10, "page": 1, "per_page": 10},
        "results": [{
            "id": "https://openalex.org/W123",
            "short_id": "works/W123",
            "display_name": "Test Work",
            "hint": "Author Name",
            "cited_by_count": 100,
            "works_count": 1,
            "entity_type": "work",
            "external_id": null,
            "filter_key": "openalex"
        }]
    }"#
    .to_string()
}

fn minimal_find_json() -> String {
    r#"{
        "meta": null,
        "results": []
    }"#
    .to_string()
}

fn make_server(mock_server: &MockServer) -> PapersMcp {
    let client = OpenAlexClient::new().with_base_url(mock_server.uri());
    PapersMcp::with_client(client)
}

// ── List tool tests ──────────────────────────────────────────────────

#[tokio::test]
async fn test_list_works_tool() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/works"))
        .respond_with(ResponseTemplate::new(200).set_body_string(minimal_list_json()))
        .mount(&mock)
        .await;

    let server = make_server(&mock);
    let params = serde_json::from_value(serde_json::json!({})).unwrap();
    let result = server.list_works(Parameters(params)).await;
    let text = result.unwrap();
    assert!(text.contains("\"count\": 42"));
}

#[tokio::test]
async fn test_list_works_with_params() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/works"))
        .and(query_param("search", "machine learning"))
        .and(query_param("per-page", "5"))
        .respond_with(ResponseTemplate::new(200).set_body_string(minimal_list_json()))
        .mount(&mock)
        .await;

    let server = make_server(&mock);
    let params = serde_json::from_value(serde_json::json!({
        "search": "machine learning",
        "per_page": 5
    }))
    .unwrap();
    let result = server.list_works(Parameters(params)).await;
    assert!(result.is_ok());
}

// ── Get tool tests ───────────────────────────────────────────────────

#[tokio::test]
async fn test_get_work_tool() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/works/W2741809807"))
        .respond_with(ResponseTemplate::new(200).set_body_string(minimal_work_json()))
        .mount(&mock)
        .await;

    let server = make_server(&mock);
    let params = serde_json::from_value(serde_json::json!({"id": "W2741809807"})).unwrap();
    let result = server.get_work(Parameters(params)).await;
    let text = result.unwrap();
    assert!(text.contains("The state of OA"));
}

#[tokio::test]
async fn test_get_work_with_select() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/works/W123"))
        .and(query_param("select", "id,display_name"))
        .respond_with(ResponseTemplate::new(200).set_body_string(minimal_work_json()))
        .mount(&mock)
        .await;

    let server = make_server(&mock);
    let params =
        serde_json::from_value(serde_json::json!({"id": "W123", "select": "id,display_name"}))
            .unwrap();
    let result = server.get_work(Parameters(params)).await;
    assert!(result.is_ok());
}

// ── Autocomplete tool tests ──────────────────────────────────────────

#[tokio::test]
async fn test_autocomplete_works_tool() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/autocomplete/works"))
        .and(query_param("q", "machine"))
        .respond_with(ResponseTemplate::new(200).set_body_string(minimal_autocomplete_json()))
        .mount(&mock)
        .await;

    let server = make_server(&mock);
    let params = serde_json::from_value(serde_json::json!({"q": "machine"})).unwrap();
    let result = server.autocomplete_works(Parameters(params)).await;
    let text = result.unwrap();
    assert!(text.contains("Test Work"));
}

// ── Find works tool tests ────────────────────────────────────────────

#[tokio::test]
async fn test_find_works_get() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/find/works"))
        .and(query_param("query", "drug discovery"))
        .respond_with(ResponseTemplate::new(200).set_body_string(minimal_find_json()))
        .mount(&mock)
        .await;

    let server = make_server(&mock);
    let params = serde_json::from_value(serde_json::json!({"query": "drug discovery"})).unwrap();
    let result = server.find_works(Parameters(params)).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_find_works_post_for_long_query() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/find/works"))
        .respond_with(ResponseTemplate::new(200).set_body_string(minimal_find_json()))
        .mount(&mock)
        .await;

    let long_query = "a ".repeat(1500); // >2048 chars
    let server = make_server(&mock);
    let params = serde_json::from_value(serde_json::json!({"query": long_query})).unwrap();
    let result = server.find_works(Parameters(params)).await;
    assert!(result.is_ok());
}

// ── Error handling tests ─────────────────────────────────────────────

#[tokio::test]
async fn test_api_error_returns_error_result() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/works/invalid"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not found"))
        .mount(&mock)
        .await;

    let server = make_server(&mock);
    let params = serde_json::from_value(serde_json::json!({"id": "invalid"})).unwrap();
    let result = server.get_work(Parameters(params)).await;

    // API errors should be returned as Err, not panics
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("404"));
}

// ── Tool listing tests ───────────────────────────────────────────────

#[test]
fn test_tool_router_has_22_tools() {
    let router = PapersMcp::tool_router();
    let tools = router.list_all();
    assert_eq!(tools.len(), 22);
}

#[test]
fn test_all_tool_names_present() {
    let router = PapersMcp::tool_router();
    let tools = router.list_all();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();

    let expected = [
        "list_works",
        "list_authors",
        "list_sources",
        "list_institutions",
        "list_topics",
        "list_publishers",
        "list_funders",
        "get_work",
        "get_author",
        "get_source",
        "get_institution",
        "get_topic",
        "get_publisher",
        "get_funder",
        "autocomplete_works",
        "autocomplete_authors",
        "autocomplete_sources",
        "autocomplete_institutions",
        "autocomplete_concepts",
        "autocomplete_publishers",
        "autocomplete_funders",
        "find_works",
    ];

    for name in &expected {
        assert!(names.contains(name), "Missing tool: {name}");
    }
}

#[test]
fn test_all_tools_have_descriptions() {
    let router = PapersMcp::tool_router();
    let tools = router.list_all();

    for tool in &tools {
        assert!(
            tool.description.is_some(),
            "Tool {} is missing a description",
            tool.name
        );
    }
}

// ── Schema tests ─────────────────────────────────────────────────────

#[test]
fn test_tool_params_schema() {
    use papers_mcp::params::ListToolParams;
    let schema = schemars::schema_for!(ListToolParams);
    let json = serde_json::to_value(&schema).unwrap();
    assert_eq!(json["type"], "object");
    let props = json["properties"].as_object().unwrap();
    assert!(props.contains_key("filter"));
    assert!(props.contains_key("search"));
    assert!(props.contains_key("sort"));
    assert!(props.contains_key("per_page"));
    assert!(props.contains_key("page"));
    assert!(props.contains_key("cursor"));
    assert!(props.contains_key("sample"));
    assert!(props.contains_key("seed"));
    assert!(props.contains_key("select"));
    assert!(props.contains_key("group_by"));
}
