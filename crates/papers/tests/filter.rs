use papers::{OpenAlexClient, WorkFilterAliases};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn make_client(mock: &MockServer) -> OpenAlexClient {
    OpenAlexClient::new().with_base_url(mock.uri())
}

/// Minimal list response with one result containing the given ID.
fn search_result_json(id: &str) -> String {
    format!(
        r#"{{
            "meta": {{"count": 1, "db_response_time_ms": 5, "page": 1, "per_page": 1, "next_cursor": null, "groups_count": null}},
            "results": [{{"id": "{id}", "display_name": "Test Entity"}}],
            "group_by": []
        }}"#
    )
}

/// Empty list response (no results).
fn empty_search_result_json() -> String {
    r#"{
        "meta": {"count": 0, "db_response_time_ms": 5, "page": 1, "per_page": 1, "next_cursor": null, "groups_count": null},
        "results": [],
        "group_by": []
    }"#
    .to_string()
}

#[tokio::test]
async fn test_resolve_author_search() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/authors"))
        .and(query_param("filter", "display_name.search:einstein"))
        .and(query_param("sort", "cited_by_count:desc"))
        .and(query_param("per-page", "1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(search_result_json("https://openalex.org/A5083138872")),
        )
        .mount(&mock)
        .await;

    let client = make_client(&mock);
    let aliases = WorkFilterAliases {
        author: Some("einstein".to_string()),
        ..Default::default()
    };
    let result = papers::resolve_work_filters(&client, &aliases, None)
        .await
        .unwrap();
    assert_eq!(
        result.as_deref(),
        Some("authorships.author.id:A5083138872")
    );
}

#[tokio::test]
async fn test_resolve_publisher_search() {
    let mock = MockServer::start().await;
    // Publishers use `search` param, NOT `filter=display_name.search:`
    Mock::given(method("GET"))
        .and(path("/publishers"))
        .and(query_param("search", "acm"))
        .and(query_param("sort", "cited_by_count:desc"))
        .and(query_param("per-page", "1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(search_result_json("https://openalex.org/P4310319798")),
        )
        .mount(&mock)
        .await;

    let client = make_client(&mock);
    let aliases = WorkFilterAliases {
        publisher: Some("acm".to_string()),
        ..Default::default()
    };
    let result = papers::resolve_work_filters(&client, &aliases, None)
        .await
        .unwrap();
    assert_eq!(
        result.as_deref(),
        Some("primary_location.source.publisher_lineage:P4310319798")
    );
}

#[tokio::test]
async fn test_resolve_source_search() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sources"))
        .and(query_param("filter", "display_name.search:siggraph"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(search_result_json("https://openalex.org/S131921510")),
        )
        .mount(&mock)
        .await;

    let client = make_client(&mock);
    let aliases = WorkFilterAliases {
        source: Some("siggraph".to_string()),
        ..Default::default()
    };
    let result = papers::resolve_work_filters(&client, &aliases, None)
        .await
        .unwrap();
    assert_eq!(
        result.as_deref(),
        Some("primary_location.source.id:S131921510")
    );
}

#[tokio::test]
async fn test_resolve_topic_search() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/topics"))
        .and(query_param("filter", "display_name.search:machine learning"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(search_result_json("https://openalex.org/T11636")),
        )
        .mount(&mock)
        .await;

    let client = make_client(&mock);
    let aliases = WorkFilterAliases {
        topic: Some("machine learning".to_string()),
        ..Default::default()
    };
    let result = papers::resolve_work_filters(&client, &aliases, None)
        .await
        .unwrap();
    assert_eq!(result.as_deref(), Some("primary_topic.id:T11636"));
}

#[tokio::test]
async fn test_resolve_domain_search() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/domains"))
        .and(query_param("filter", "display_name.search:physical"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(search_result_json("https://openalex.org/domains/3")),
        )
        .mount(&mock)
        .await;

    let client = make_client(&mock);
    let aliases = WorkFilterAliases {
        domain: Some("physical".to_string()),
        ..Default::default()
    };
    let result = papers::resolve_work_filters(&client, &aliases, None)
        .await
        .unwrap();
    assert_eq!(
        result.as_deref(),
        Some("primary_topic.domain.id:domains/3")
    );
}

#[tokio::test]
async fn test_resolve_field_search() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/fields"))
        .and(query_param(
            "filter",
            "display_name.search:computer science",
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(search_result_json("https://openalex.org/fields/17")),
        )
        .mount(&mock)
        .await;

    let client = make_client(&mock);
    let aliases = WorkFilterAliases {
        field: Some("computer science".to_string()),
        ..Default::default()
    };
    let result = papers::resolve_work_filters(&client, &aliases, None)
        .await
        .unwrap();
    assert_eq!(
        result.as_deref(),
        Some("primary_topic.field.id:fields/17")
    );
}

#[tokio::test]
async fn test_resolve_subfield_search() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/subfields"))
        .and(query_param(
            "filter",
            "display_name.search:artificial intelligence",
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(search_result_json("https://openalex.org/subfields/1702")),
        )
        .mount(&mock)
        .await;

    let client = make_client(&mock);
    let aliases = WorkFilterAliases {
        subfield: Some("artificial intelligence".to_string()),
        ..Default::default()
    };
    let result = papers::resolve_work_filters(&client, &aliases, None)
        .await
        .unwrap();
    assert_eq!(
        result.as_deref(),
        Some("primary_topic.subfield.id:subfields/1702")
    );
}

#[tokio::test]
async fn test_resolve_mixed_id_and_search() {
    let mock = MockServer::start().await;
    // "acm" should trigger a search, "P4310320595" should pass through
    Mock::given(method("GET"))
        .and(path("/publishers"))
        .and(query_param("search", "acm"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(search_result_json("https://openalex.org/P4310319798")),
        )
        .mount(&mock)
        .await;

    let client = make_client(&mock);
    let aliases = WorkFilterAliases {
        publisher: Some("acm|P4310320595".to_string()),
        ..Default::default()
    };
    let result = papers::resolve_work_filters(&client, &aliases, None)
        .await
        .unwrap();
    assert_eq!(
        result.as_deref(),
        Some("primary_location.source.publisher_lineage:P4310319798|P4310320595")
    );
}

#[tokio::test]
async fn test_resolve_not_found_error() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/authors"))
        .respond_with(ResponseTemplate::new(200).set_body_string(empty_search_result_json()))
        .mount(&mock)
        .await;

    let client = make_client(&mock);
    let aliases = WorkFilterAliases {
        author: Some("nonexistent_person_xyz".to_string()),
        ..Default::default()
    };
    let result = papers::resolve_work_filters(&client, &aliases, None).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("nonexistent_person_xyz"));
    assert!(err.contains("authors"));
}

#[tokio::test]
async fn test_resolve_overlap_error() {
    let mock = MockServer::start().await;
    // No mocks needed — overlap is checked before any API calls
    let client = make_client(&mock);
    let aliases = WorkFilterAliases {
        year: Some("2024".to_string()),
        ..Default::default()
    };
    let result =
        papers::resolve_work_filters(&client, &aliases, Some("publication_year:>2020")).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("year"));
    assert!(err.contains("publication_year"));
}

#[tokio::test]
async fn test_resolve_all_aliases_combined() {
    let mock = MockServer::start().await;

    // Mock author search
    Mock::given(method("GET"))
        .and(path("/authors"))
        .and(query_param("filter", "display_name.search:einstein"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(search_result_json("https://openalex.org/A5083138872")),
        )
        .mount(&mock)
        .await;

    // Mock source search
    Mock::given(method("GET"))
        .and(path("/sources"))
        .and(query_param("filter", "display_name.search:nature"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(search_result_json("https://openalex.org/S123456")),
        )
        .mount(&mock)
        .await;

    let client = make_client(&mock);
    let aliases = WorkFilterAliases {
        author: Some("einstein".to_string()),
        topic: Some("T11636".to_string()),
        domain: Some("3".to_string()),
        source: Some("nature".to_string()),
        year: Some(">2020".to_string()),
        citations: Some(">100".to_string()),
        ..Default::default()
    };
    let result = papers::resolve_work_filters(&client, &aliases, Some("is_oa:true"))
        .await
        .unwrap();
    let filter = result.unwrap();

    assert!(filter.contains("authorships.author.id:A5083138872"));
    assert!(filter.contains("primary_topic.id:T11636"));
    assert!(filter.contains("primary_topic.domain.id:domains/3"));
    assert!(filter.contains("primary_location.source.id:S123456"));
    assert!(filter.contains("publication_year:>2020"));
    assert!(filter.contains("cited_by_count:>100"));
    assert!(filter.contains("is_oa:true"));
}
