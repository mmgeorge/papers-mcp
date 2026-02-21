use serde::{Deserialize, Serialize};

/// Full-text content for a single attachment item.
///
/// Returned by `GET /users/<id>/items/<key>/fulltext`.
///
/// PDF attachments report `indexed_pages` / `total_pages`.
/// Non-PDF documents (HTML, EPUB, etc.) report `indexed_chars` / `total_chars`
/// instead. Either pair may be absent depending on the indexer used.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemFulltext {
    /// Extracted plain-text content of the attachment.
    pub content: String,

    /// Number of pages that were successfully indexed (PDF).
    #[serde(rename = "indexedPages")]
    pub indexed_pages: Option<u32>,

    /// Total number of pages in the document (PDF).
    #[serde(rename = "totalPages")]
    pub total_pages: Option<u32>,

    /// Number of characters that were successfully indexed (non-PDF).
    #[serde(rename = "indexedChars")]
    pub indexed_chars: Option<u32>,

    /// Total number of characters in the document (non-PDF).
    #[serde(rename = "totalChars")]
    pub total_chars: Option<u32>,
}
