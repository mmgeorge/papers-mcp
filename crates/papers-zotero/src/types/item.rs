use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::common::*;

/// A Zotero library item (journal article, book, attachment, note, etc.).
///
/// Items are the primary entity in Zotero. Different item types have different
/// data fields — the `data` field uses `#[serde(flatten)]` to capture all
/// type-specific fields dynamically.
///
/// # Single item response
///
/// `GET /users/<id>/items/<key>` returns a single `Item` object (not an array).
///
/// # List response
///
/// `GET /users/<id>/items` returns a JSON array `[Item, ...]`.
///
/// # Example (journalArticle)
///
/// ```json
/// {
///     "key": "LF4MJWZK",
///     "version": 4348,
///     "library": { "type": "user", "id": 16916553, "name": "mattmg", "links": {} },
///     "links": { "self": { "href": "...", "type": "application/json" } },
///     "meta": { "creatorSummary": "Doe et al.", "numChildren": 1 },
///     "data": {
///         "key": "LF4MJWZK",
///         "version": 4348,
///         "itemType": "journalArticle",
///         "title": "Example Title",
///         "creators": [{ "creatorType": "author", "firstName": "John", "lastName": "Doe" }],
///         "tags": [{ "tag": "ML" }],
///         "collections": ["BDGZ4NHT"],
///         "dateAdded": "2026-02-16T02:13:13Z",
///         "dateModified": "2026-02-16T02:13:32Z"
///     }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub key: String,
    pub version: u64,
    pub library: Library,
    #[serde(default)]
    pub links: HashMap<String, LinkEntry>,
    #[serde(default)]
    pub meta: ItemMeta,
    pub data: ItemData,
}

/// Item metadata returned outside of `data` (in the `meta` object).
///
/// ```json
/// { "creatorSummary": "Doe et al.", "numChildren": 1 }
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ItemMeta {
    #[serde(rename = "creatorSummary")]
    pub creator_summary: Option<String>,
    #[serde(rename = "numChildren")]
    pub num_children: Option<u64>,
    #[serde(rename = "parsedDate")]
    pub parsed_date: Option<String>,
}

/// Item data payload. Contains all bibliographic fields.
///
/// Since different item types (journalArticle, book, attachment, note, etc.)
/// have different fields, only the common fields are typed explicitly. All
/// remaining type-specific fields are captured in `extra_fields`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemData {
    pub key: String,
    pub version: u64,
    #[serde(rename = "itemType")]
    pub item_type: String,
    pub title: Option<String>,
    #[serde(default)]
    pub creators: Vec<Creator>,
    #[serde(default)]
    pub tags: Vec<ItemTag>,
    #[serde(default)]
    pub collections: Vec<String>,
    pub relations: Option<serde_json::Value>,
    #[serde(rename = "dateAdded")]
    pub date_added: Option<String>,
    #[serde(rename = "dateModified")]
    pub date_modified: Option<String>,

    // ── Common bibliographic fields ───────────────────────────────────

    #[serde(rename = "abstractNote")]
    pub abstract_note: Option<String>,
    #[serde(rename = "publicationTitle")]
    pub publication_title: Option<String>,
    pub publisher: Option<String>,
    pub place: Option<String>,
    pub date: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub pages: Option<String>,
    pub series: Option<String>,
    #[serde(rename = "seriesTitle")]
    pub series_title: Option<String>,
    #[serde(rename = "seriesText")]
    pub series_text: Option<String>,
    #[serde(rename = "journalAbbreviation")]
    pub journal_abbreviation: Option<String>,
    #[serde(rename = "DOI")]
    pub doi: Option<String>,
    #[serde(rename = "citationKey")]
    pub citation_key: Option<String>,
    pub url: Option<String>,
    #[serde(rename = "accessDate")]
    pub access_date: Option<String>,
    pub archive: Option<String>,
    #[serde(rename = "archiveLocation")]
    pub archive_location: Option<String>,
    #[serde(rename = "shortTitle")]
    pub short_title: Option<String>,
    pub language: Option<String>,
    #[serde(rename = "libraryCatalog")]
    pub library_catalog: Option<String>,
    #[serde(rename = "callNumber")]
    pub call_number: Option<String>,
    pub rights: Option<String>,
    pub extra: Option<String>,
    #[serde(rename = "ISBN")]
    pub isbn: Option<String>,
    #[serde(rename = "ISSN")]
    pub issn: Option<String>,
    #[serde(rename = "PMID")]
    pub pmid: Option<String>,
    #[serde(rename = "PMCID")]
    pub pmcid: Option<String>,

    // ── Attachment-specific fields ────────────────────────────────────

    #[serde(rename = "parentItem")]
    pub parent_item: Option<String>,
    #[serde(rename = "linkMode")]
    pub link_mode: Option<String>,
    #[serde(rename = "contentType")]
    pub content_type: Option<String>,
    pub charset: Option<String>,
    pub filename: Option<String>,
    pub md5: Option<String>,
    pub mtime: Option<u64>,

    // ── Note-specific fields ─────────────────────────────────────────

    pub note: Option<String>,

    // ── Additional fields (section, partNumber, etc.) ────────────────

    pub section: Option<String>,
    #[serde(rename = "partNumber")]
    pub part_number: Option<String>,
    #[serde(rename = "partTitle")]
    pub part_title: Option<String>,

    /// All remaining type-specific fields not explicitly listed above.
    #[serde(flatten)]
    pub extra_fields: HashMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_item_fixture() {
        let json = include_str!("../../tests/fixtures/item.json");
        let item: Item = serde_json::from_str(json).unwrap();
        assert_eq!(item.key, "LF4MJWZK");
        assert_eq!(item.data.item_type, "journalArticle");
        assert_eq!(item.data.title.as_deref(), Some("Real-Time Rendering 4th: Ray Tracing"));
        assert_eq!(item.data.creators.len(), 6);
        assert_eq!(item.data.creators[0].creator_type, "author");
        assert_eq!(item.data.creators[0].last_name.as_deref(), Some("Akenine-Moller"));
    }

    #[test]
    fn test_deserialize_attachment_fixture() {
        let json = include_str!("../../tests/fixtures/attachment.json");
        let item: Item = serde_json::from_str(json).unwrap();
        assert_eq!(item.data.item_type, "attachment");
        assert_eq!(item.data.link_mode.as_deref(), Some("imported_file"));
        assert!(item.data.filename.is_some());
    }
}
