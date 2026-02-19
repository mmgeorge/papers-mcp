use papers_openalex::{
    Author, AutocompleteResponse, Domain, Field, FindWorksParams, FindWorksResponse, Funder,
    GetParams, Institution, OpenAlexClient, OpenAlexError, Publisher, Source, Subfield,
    Topic, Work,
};

use crate::filter::{
    AuthorListParams, DomainListParams, FieldListParams, FilterError, FunderListParams,
    InstitutionListParams, PublisherListParams, SourceListParams, SubfieldListParams,
    TopicListParams, WorkListParams, is_openalex_id, resolve_entity_id,
    resolve_filters, WORK_ALIASES,
};
use crate::summary::{
    AuthorSummary, DomainSummary, FieldSummary, FunderSummary, InstitutionSummary,
    PublisherSummary, SlimListResponse, SourceSummary, SubfieldSummary, TopicSummary, WorkSummary,
    summary_list_result,
};

// ── List ─────────────────────────────────────────────────────────────────

pub async fn work_list(
    client: &OpenAlexClient,
    params: &WorkListParams,
) -> Result<SlimListResponse<WorkSummary>, FilterError> {
    let (alias_values, mut list_params) = params.into_aliases_and_list_params();
    list_params.filter = resolve_filters(client, WORK_ALIASES, &alias_values, list_params.filter.as_deref()).await?;
    Ok(summary_list_result(client.list_works(&list_params).await, WorkSummary::from)?)
}

macro_rules! entity_list_fn {
    ($fn_name:ident, $params_type:ident, $summary_type:ident, $client_method:ident) => {
        pub async fn $fn_name(
            client: &OpenAlexClient,
            params: &$params_type,
        ) -> Result<SlimListResponse<$summary_type>, FilterError> {
            let (alias_values, mut list_params) = params.into_aliases_and_list_params();
            list_params.filter = resolve_filters(
                client,
                $params_type::alias_specs(),
                &alias_values,
                list_params.filter.as_deref(),
            ).await?;
            Ok(summary_list_result(client.$client_method(&list_params).await, $summary_type::from)?)
        }
    };
}

entity_list_fn!(author_list, AuthorListParams, AuthorSummary, list_authors);
entity_list_fn!(source_list, SourceListParams, SourceSummary, list_sources);
entity_list_fn!(institution_list, InstitutionListParams, InstitutionSummary, list_institutions);
entity_list_fn!(topic_list, TopicListParams, TopicSummary, list_topics);
entity_list_fn!(publisher_list, PublisherListParams, PublisherSummary, list_publishers);
entity_list_fn!(funder_list, FunderListParams, FunderSummary, list_funders);
entity_list_fn!(domain_list, DomainListParams, DomainSummary, list_domains);
entity_list_fn!(field_list, FieldListParams, FieldSummary, list_fields);
entity_list_fn!(subfield_list, SubfieldListParams, SubfieldSummary, list_subfields);

// ── Get (smart ID resolution) ────────────────────────────────────────────

/// Returns `true` if `input` looks like a known identifier for the given entity type
/// (as opposed to a search query that needs resolution).
fn looks_like_identifier(input: &str, entity_type: &str) -> bool {
    // OpenAlex IDs (short or full URL)
    if is_openalex_id(input, entity_type) {
        return true;
    }

    // DOIs (works only, but safe to pass through for any entity — API will 404 gracefully)
    if input.starts_with("https://doi.org/")
        || input.starts_with("doi:")
        || (input.starts_with("10.") && input.contains('/'))
    {
        return true;
    }

    // PubMed IDs
    if input.starts_with("pmid:") || input.starts_with("pmcid:") {
        return true;
    }

    // ORCIDs (authors)
    if input.starts_with("https://orcid.org/") {
        return true;
    }

    // ROR IDs (institutions)
    if input.starts_with("https://ror.org/") {
        return true;
    }

    // ISSNs: XXXX-XXXX pattern (digits and X)
    if input.len() == 9 && input.as_bytes().get(4) == Some(&b'-') {
        let (left, right) = (&input[..4], &input[5..]);
        if left.chars().all(|c| c.is_ascii_digit() || c == 'X' || c == 'x')
            && right.chars().all(|c| c.is_ascii_digit() || c == 'X' || c == 'x')
        {
            return true;
        }
    }

    false
}

/// Strip any well-known ID prefixes to get the bare ID for a get endpoint.
///
/// Unlike `normalize_id` (which adds path prefixes for filter expressions),
/// this strips prefixes to get the bare form the client's get methods expect:
/// - `https://openalex.org/W123` → `W123`
/// - `https://openalex.org/domains/3` → `3`
/// - `domains/3` → `3`
/// - `W123` → `W123` (unchanged)
/// - `10.1234/foo` → `doi:10.1234/foo` (bare DOI needs prefix for OpenAlex API)
fn bare_id_for_get(input: &str, entity_type: &str) -> String {
    // Strip full OpenAlex URL prefix
    let id = input.strip_prefix("https://openalex.org/").unwrap_or(input);
    // For hierarchy entities, the client adds the path itself — strip it
    let id = match entity_type {
        "domains" => id.strip_prefix("domains/").unwrap_or(id),
        "fields" => id.strip_prefix("fields/").unwrap_or(id),
        "subfields" => id.strip_prefix("subfields/").unwrap_or(id),
        _ => id,
    };
    // Bare DOIs (e.g. "10.1234/foo") need the "doi:" prefix for the OpenAlex /works/{id}
    // endpoint. DOIs already prefixed with "doi:" or "https://doi.org/" are left unchanged.
    if id.starts_with("10.") && id.contains('/') {
        return format!("doi:{id}");
    }
    id.to_string()
}

/// Resolve an input string to an entity ID suitable for the get endpoint.
///
/// If the input looks like a known identifier, it is returned in bare form.
/// Otherwise, it is treated as a search query: the list endpoint is queried
/// for the top result by citation count, and that result's ID is used.
async fn resolve_get_id(
    client: &OpenAlexClient,
    input: &str,
    entity_type: &'static str,
) -> Result<String, FilterError> {
    if looks_like_identifier(input, entity_type) {
        Ok(bare_id_for_get(input, entity_type))
    } else {
        // resolve_entity_id returns a normalized ID (e.g. "domains/3"); strip for get
        let normalized = resolve_entity_id(client, input, entity_type).await?;
        Ok(bare_id_for_get(&normalized, entity_type))
    }
}

macro_rules! entity_get_fn {
    ($fn_name:ident, $return_type:ident, $client_method:ident, $entity_type:literal) => {
        pub async fn $fn_name(
            client: &OpenAlexClient,
            id: &str,
            params: &GetParams,
        ) -> Result<$return_type, FilterError> {
            let resolved = resolve_get_id(client, id, $entity_type).await?;
            Ok(client.$client_method(&resolved, params).await?)
        }
    };
}

entity_get_fn!(work_get, Work, get_work, "works");
entity_get_fn!(author_get, Author, get_author, "authors");
entity_get_fn!(source_get, Source, get_source, "sources");
entity_get_fn!(institution_get, Institution, get_institution, "institutions");
entity_get_fn!(topic_get, Topic, get_topic, "topics");
entity_get_fn!(publisher_get, Publisher, get_publisher, "publishers");
entity_get_fn!(funder_get, Funder, get_funder, "funders");
entity_get_fn!(domain_get, Domain, get_domain, "domains");
entity_get_fn!(field_get, Field, get_field, "fields");
entity_get_fn!(subfield_get, Subfield, get_subfield, "subfields");

// ── Autocomplete ─────────────────────────────────────────────────────────

pub async fn work_autocomplete(
    client: &OpenAlexClient,
    q: &str,
) -> Result<AutocompleteResponse, OpenAlexError> {
    client.autocomplete_works(q).await
}

pub async fn author_autocomplete(
    client: &OpenAlexClient,
    q: &str,
) -> Result<AutocompleteResponse, OpenAlexError> {
    client.autocomplete_authors(q).await
}

pub async fn source_autocomplete(
    client: &OpenAlexClient,
    q: &str,
) -> Result<AutocompleteResponse, OpenAlexError> {
    client.autocomplete_sources(q).await
}

pub async fn institution_autocomplete(
    client: &OpenAlexClient,
    q: &str,
) -> Result<AutocompleteResponse, OpenAlexError> {
    client.autocomplete_institutions(q).await
}

pub async fn publisher_autocomplete(
    client: &OpenAlexClient,
    q: &str,
) -> Result<AutocompleteResponse, OpenAlexError> {
    client.autocomplete_publishers(q).await
}

pub async fn funder_autocomplete(
    client: &OpenAlexClient,
    q: &str,
) -> Result<AutocompleteResponse, OpenAlexError> {
    client.autocomplete_funders(q).await
}

pub async fn subfield_autocomplete(
    client: &OpenAlexClient,
    q: &str,
) -> Result<AutocompleteResponse, OpenAlexError> {
    client.autocomplete_subfields(q).await
}

// ── Find ─────────────────────────────────────────────────────────────────

/// AI semantic search for works by conceptual similarity.
/// Automatically uses POST for queries longer than 2048 characters.
pub async fn work_find(
    client: &OpenAlexClient,
    params: &FindWorksParams,
) -> Result<FindWorksResponse, OpenAlexError> {
    if params.query.len() > 2048 {
        client.find_works_post(params).await
    } else {
        client.find_works(params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── looks_like_identifier: OpenAlex IDs ──────────────────────────────

    #[test]
    fn id_work_short() {
        assert!(looks_like_identifier("W2741809807", "works"));
    }

    #[test]
    fn id_author_short() {
        assert!(looks_like_identifier("A5023888391", "authors"));
    }

    #[test]
    fn id_source_short() {
        assert!(looks_like_identifier("S131921510", "sources"));
    }

    #[test]
    fn id_institution_short() {
        assert!(looks_like_identifier("I136199984", "institutions"));
    }

    #[test]
    fn id_topic_short() {
        assert!(looks_like_identifier("T11636", "topics"));
    }

    #[test]
    fn id_publisher_short() {
        assert!(looks_like_identifier("P4310319798", "publishers"));
    }

    #[test]
    fn id_funder_short() {
        assert!(looks_like_identifier("F1234567", "funders"));
    }

    #[test]
    fn id_full_openalex_url() {
        assert!(looks_like_identifier("https://openalex.org/W2741809807", "works"));
        assert!(looks_like_identifier("https://openalex.org/A123", "authors"));
    }

    // ── looks_like_identifier: DOIs ──────────────────────────────────────

    #[test]
    fn id_doi_url() {
        assert!(looks_like_identifier("https://doi.org/10.1109/ipdps.2012.30", "works"));
    }

    #[test]
    fn id_doi_prefix() {
        assert!(looks_like_identifier("doi:10.1109/ipdps.2012.30", "works"));
    }

    #[test]
    fn id_bare_doi() {
        assert!(looks_like_identifier("10.1109/ipdps.2012.30", "works"));
    }

    // ── looks_like_identifier: PubMed IDs ────────────────────────────────

    #[test]
    fn id_pmid() {
        assert!(looks_like_identifier("pmid:12345678", "works"));
    }

    #[test]
    fn id_pmcid() {
        assert!(looks_like_identifier("pmcid:PMC1234567", "works"));
    }

    // ── looks_like_identifier: ORCIDs ────────────────────────────────────

    #[test]
    fn id_orcid() {
        assert!(looks_like_identifier("https://orcid.org/0000-0002-1825-0097", "authors"));
    }

    // ── looks_like_identifier: ROR IDs ───────────────────────────────────

    #[test]
    fn id_ror() {
        assert!(looks_like_identifier("https://ror.org/03vek6s52", "institutions"));
    }

    // ── looks_like_identifier: ISSNs ─────────────────────────────────────

    #[test]
    fn id_issn() {
        assert!(looks_like_identifier("0028-0836", "sources"));
    }

    #[test]
    fn id_issn_with_x() {
        assert!(looks_like_identifier("0000-000X", "sources"));
    }

    // ── looks_like_identifier: hierarchy IDs ─────────────────────────────

    #[test]
    fn id_domain_bare_digits() {
        assert!(looks_like_identifier("3", "domains"));
    }

    #[test]
    fn id_domain_path() {
        assert!(looks_like_identifier("domains/3", "domains"));
    }

    #[test]
    fn id_field_bare_digits() {
        assert!(looks_like_identifier("17", "fields"));
    }

    #[test]
    fn id_subfield_bare_digits() {
        assert!(looks_like_identifier("1702", "subfields"));
    }

    // ── looks_like_identifier: search queries (should be false) ──────────

    #[test]
    fn search_query_title() {
        assert!(!looks_like_identifier("adaptive bitonic sort", "works"));
    }

    #[test]
    fn search_query_author_name() {
        assert!(!looks_like_identifier("Albert Einstein", "authors"));
    }

    #[test]
    fn search_query_source_name() {
        assert!(!looks_like_identifier("Nature", "sources"));
    }

    #[test]
    fn search_query_institution_name() {
        assert!(!looks_like_identifier("Massachusetts Institute of Technology", "institutions"));
    }

    #[test]
    fn search_query_topic_name() {
        assert!(!looks_like_identifier("machine learning", "topics"));
    }

    #[test]
    fn search_query_publisher_name() {
        assert!(!looks_like_identifier("Elsevier", "publishers"));
    }

    #[test]
    fn search_query_funder_name() {
        assert!(!looks_like_identifier("National Science Foundation", "funders"));
    }

    #[test]
    fn search_query_domain_name() {
        assert!(!looks_like_identifier("Physical Sciences", "domains"));
    }
}
