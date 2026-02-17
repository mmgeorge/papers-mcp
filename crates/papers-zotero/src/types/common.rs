use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Library that owns an item, collection, or search.
///
/// ```json
/// {
///     "type": "user",
///     "id": 16916553,
///     "name": "mattmg",
///     "links": {
///         "alternate": { "href": "https://www.zotero.org/mattmg", "type": "text/html" }
///     }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    #[serde(rename = "type")]
    pub library_type: String,
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub links: HashMap<String, LinkEntry>,
}

/// A single link entry with href and content type.
///
/// Some link entries have additional fields like `attachmentType` and
/// `attachmentSize` (for attachment links on items).
///
/// ```json
/// { "href": "https://api.zotero.org/...", "type": "application/json" }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkEntry {
    pub href: String,
    #[serde(rename = "type")]
    pub content_type: Option<String>,
    /// MIME type of the attachment (present on item attachment links).
    #[serde(rename = "attachmentType")]
    pub attachment_type: Option<String>,
    /// Size in bytes of the attachment (present on item attachment links).
    #[serde(rename = "attachmentSize")]
    pub attachment_size: Option<u64>,
}

/// A creator (author, editor, etc.) on an item.
///
/// Creators have either `firstName`+`lastName` or a single `name` field
/// (for institutional authors).
///
/// ```json
/// { "creatorType": "author", "firstName": "Tomas", "lastName": "Akenine-Moller" }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Creator {
    #[serde(rename = "creatorType")]
    pub creator_type: String,
    #[serde(rename = "firstName")]
    pub first_name: Option<String>,
    #[serde(rename = "lastName")]
    pub last_name: Option<String>,
    /// Single-field name (used for institutional authors).
    pub name: Option<String>,
}

/// A tag attached to an item.
///
/// ```json
/// { "tag": "machine learning", "type": 0 }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemTag {
    pub tag: String,
    /// Tag type: 0 = user-created, 1 = automatic/imported. May be absent.
    #[serde(rename = "type", default)]
    pub tag_type: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_library() {
        let json = r#"{
            "type": "user",
            "id": 16916553,
            "name": "mattmg",
            "links": {
                "alternate": { "href": "https://www.zotero.org/mattmg", "type": "text/html" }
            }
        }"#;
        let lib: Library = serde_json::from_str(json).unwrap();
        assert_eq!(lib.library_type, "user");
        assert_eq!(lib.id, 16916553);
        assert_eq!(lib.name, "mattmg");
    }

    #[test]
    fn test_deserialize_creator_with_name_parts() {
        let json = r#"{"creatorType": "author", "firstName": "John", "lastName": "Doe"}"#;
        let c: Creator = serde_json::from_str(json).unwrap();
        assert_eq!(c.creator_type, "author");
        assert_eq!(c.first_name.as_deref(), Some("John"));
    }

    #[test]
    fn test_deserialize_creator_with_single_name() {
        let json = r#"{"creatorType": "author", "name": "NVIDIA Corporation"}"#;
        let c: Creator = serde_json::from_str(json).unwrap();
        assert_eq!(c.name.as_deref(), Some("NVIDIA Corporation"));
        assert!(c.first_name.is_none());
    }

    #[test]
    fn test_deserialize_item_tag() {
        let json = r#"{"tag": "ML", "type": 0}"#;
        let t: ItemTag = serde_json::from_str(json).unwrap();
        assert_eq!(t.tag, "ML");
        assert_eq!(t.tag_type, Some(0));
    }

    #[test]
    fn test_deserialize_item_tag_no_type() {
        let json = r#"{"tag": "ML"}"#;
        let t: ItemTag = serde_json::from_str(json).unwrap();
        assert_eq!(t.tag, "ML");
        assert_eq!(t.tag_type, None);
    }
}
