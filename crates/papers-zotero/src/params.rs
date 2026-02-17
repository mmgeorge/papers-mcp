/// Query parameters for item list endpoints (`/items`, `/items/top`,
/// `/collections/<key>/items`, etc.). All fields are optional.
///
/// ```
/// use papers_zotero::ItemListParams;
///
/// let params = ItemListParams::builder()
///     .q("machine learning")
///     .item_type("journalArticle")
///     .sort("dateModified")
///     .direction("desc")
///     .limit(25)
///     .build();
/// ```
#[derive(Debug, Default, Clone, bon::Builder)]
#[builder(on(String, into))]
pub struct ItemListParams {
    /// Quick search query. Searches title, creators, and year.
    pub q: Option<String>,

    /// Search mode: `titleCreatorYear` (default) or `everything`.
    pub qmode: Option<String>,

    /// Filter by tag name. Can be combined with `||` for OR, `-` prefix for
    /// NOT. Multiple `tag` parameters create AND conditions.
    pub tag: Option<String>,

    /// Filter by item type (e.g. `journalArticle`, `book`, `conferencePaper`).
    /// Prefix with `-` to exclude. Use `||` for OR.
    #[builder(name = "item_type")]
    pub item_type: Option<String>,

    /// Filter to specific item keys (comma-separated, max 50).
    #[builder(name = "item_key")]
    pub item_key: Option<String>,

    /// Only return items modified after this library version. Used for
    /// incremental sync.
    pub since: Option<u64>,

    /// Sort field: `dateAdded`, `dateModified`, `title`, `creator`,
    /// `itemType`, `date`, `publisher`, `publicationTitle`, `journalAbbreviation`,
    /// `language`, `accessDate`, `libraryCatalog`, `callNumber`, `rights`,
    /// `addedBy`, `numItems`.
    pub sort: Option<String>,

    /// Sort direction: `asc` or `desc`.
    pub direction: Option<String>,

    /// Maximum number of results (1-100, default 25).
    pub limit: Option<u32>,

    /// Offset for pagination (0-based).
    pub start: Option<u32>,

    /// Response format: `json` (default), `keys`, `versions`, `bibtex`,
    /// `csljson`, `ris`, etc.
    pub format: Option<String>,

    /// Comma-separated data to include: `bib`, `citation`, `data`,
    /// `csljson`. Only applies when `format=json`.
    pub include: Option<String>,

    /// Citation style for `include=bib` or `include=citation`.
    pub style: Option<String>,

    /// Include trashed items in results.
    #[builder(name = "include_trashed")]
    pub include_trashed: Option<bool>,
}

impl ItemListParams {
    pub(crate) fn to_query_pairs(&self) -> Vec<(&str, String)> {
        let mut pairs = Vec::new();
        if let Some(v) = &self.q {
            pairs.push(("q", v.clone()));
        }
        if let Some(v) = &self.qmode {
            pairs.push(("qmode", v.clone()));
        }
        if let Some(v) = &self.tag {
            pairs.push(("tag", v.clone()));
        }
        if let Some(v) = &self.item_type {
            pairs.push(("itemType", v.clone()));
        }
        if let Some(v) = &self.item_key {
            pairs.push(("itemKey", v.clone()));
        }
        if let Some(v) = self.since {
            pairs.push(("since", v.to_string()));
        }
        if let Some(v) = &self.sort {
            pairs.push(("sort", v.clone()));
        }
        if let Some(v) = &self.direction {
            pairs.push(("direction", v.clone()));
        }
        if let Some(v) = self.limit {
            pairs.push(("limit", v.to_string()));
        }
        if let Some(v) = self.start {
            pairs.push(("start", v.to_string()));
        }
        if let Some(v) = &self.format {
            pairs.push(("format", v.clone()));
        }
        if let Some(v) = &self.include {
            pairs.push(("include", v.clone()));
        }
        if let Some(v) = &self.style {
            pairs.push(("style", v.clone()));
        }
        if let Some(v) = self.include_trashed {
            pairs.push(("includeTrashed", if v { "1" } else { "0" }.into()));
        }
        pairs
    }
}

/// Query parameters for collection list endpoints (`/collections`,
/// `/collections/top`, `/collections/<key>/collections`).
///
/// ```
/// use papers_zotero::CollectionListParams;
///
/// let params = CollectionListParams::builder()
///     .sort("title")
///     .limit(50)
///     .build();
/// ```
#[derive(Debug, Default, Clone, bon::Builder)]
#[builder(on(String, into))]
pub struct CollectionListParams {
    /// Sort field: `title`, `dateAdded`, `dateModified`.
    pub sort: Option<String>,

    /// Sort direction: `asc` or `desc`.
    pub direction: Option<String>,

    /// Maximum number of results (1-100, default 25).
    pub limit: Option<u32>,

    /// Offset for pagination (0-based).
    pub start: Option<u32>,
}

impl CollectionListParams {
    pub(crate) fn to_query_pairs(&self) -> Vec<(&str, String)> {
        let mut pairs = Vec::new();
        if let Some(v) = &self.sort {
            pairs.push(("sort", v.clone()));
        }
        if let Some(v) = &self.direction {
            pairs.push(("direction", v.clone()));
        }
        if let Some(v) = self.limit {
            pairs.push(("limit", v.to_string()));
        }
        if let Some(v) = self.start {
            pairs.push(("start", v.to_string()));
        }
        pairs
    }
}

/// Query parameters for tag list endpoints (`/tags`, `/items/tags`, etc.).
///
/// ```
/// use papers_zotero::TagListParams;
///
/// let params = TagListParams::builder()
///     .q("machine")
///     .limit(50)
///     .build();
/// ```
#[derive(Debug, Default, Clone, bon::Builder)]
#[builder(on(String, into))]
pub struct TagListParams {
    /// Quick search query for tag names.
    pub q: Option<String>,

    /// Search mode: `startsWith` or `contains`.
    pub qmode: Option<String>,

    /// Maximum number of results.
    pub limit: Option<u32>,

    /// Offset for pagination (0-based).
    pub start: Option<u32>,

    /// Sort field.
    pub sort: Option<String>,

    /// Sort direction: `asc` or `desc`.
    pub direction: Option<String>,
}

impl TagListParams {
    pub(crate) fn to_query_pairs(&self) -> Vec<(&str, String)> {
        let mut pairs = Vec::new();
        if let Some(v) = &self.q {
            pairs.push(("q", v.clone()));
        }
        if let Some(v) = &self.qmode {
            pairs.push(("qmode", v.clone()));
        }
        if let Some(v) = self.limit {
            pairs.push(("limit", v.to_string()));
        }
        if let Some(v) = self.start {
            pairs.push(("start", v.to_string()));
        }
        if let Some(v) = &self.sort {
            pairs.push(("sort", v.clone()));
        }
        if let Some(v) = &self.direction {
            pairs.push(("direction", v.clone()));
        }
        pairs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_list_params_default() {
        let params = ItemListParams::default();
        assert!(params.q.is_none());
        assert!(params.sort.is_none());
        assert!(params.limit.is_none());
    }

    #[test]
    fn test_item_list_params_builder() {
        let params = ItemListParams::builder()
            .q("test")
            .item_type("journalArticle")
            .sort("dateModified")
            .direction("desc")
            .limit(10)
            .build();
        assert_eq!(params.q.as_deref(), Some("test"));
        assert_eq!(params.item_type.as_deref(), Some("journalArticle"));
        assert_eq!(params.limit, Some(10));
    }

    #[test]
    fn test_item_list_params_query_pairs() {
        let params = ItemListParams::builder()
            .q("test")
            .item_type("book")
            .limit(5)
            .include_trashed(true)
            .build();
        let pairs = params.to_query_pairs();
        assert!(pairs.contains(&("q", "test".into())));
        assert!(pairs.contains(&("itemType", "book".into())));
        assert!(pairs.contains(&("limit", "5".into())));
        assert!(pairs.contains(&("includeTrashed", "1".into())));
    }

    #[test]
    fn test_collection_list_params_builder() {
        let params = CollectionListParams::builder()
            .sort("title")
            .limit(50)
            .build();
        assert_eq!(params.sort.as_deref(), Some("title"));
        assert_eq!(params.limit, Some(50));
    }

    #[test]
    fn test_tag_list_params_builder() {
        let params = TagListParams::builder()
            .q("ML")
            .limit(10)
            .build();
        assert_eq!(params.q.as_deref(), Some("ML"));
        assert_eq!(params.limit, Some(10));
    }
}
