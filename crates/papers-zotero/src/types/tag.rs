use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::common::LinkEntry;

/// A Zotero tag (label applied to items).
///
/// Tags have two types: user-created (type 0) and automatic/imported (type 1).
/// The tag name is the primary identifier — there are no separate tag keys.
///
/// # Example
///
/// ```json
/// {
///     "tag": "machine learning",
///     "links": {
///         "self": { "href": "https://api.zotero.org/users/.../tags/machine+learning", "type": "application/json" }
///     },
///     "meta": { "type": 0, "numItems": 5 }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub tag: String,
    #[serde(default)]
    pub links: HashMap<String, LinkEntry>,
    #[serde(default)]
    pub meta: TagMeta,
}

/// Tag metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TagMeta {
    /// Tag type: 0 = user-created, 1 = automatic/imported.
    #[serde(rename = "type")]
    pub tag_type: Option<i32>,
    /// Number of items with this tag.
    #[serde(rename = "numItems")]
    pub num_items: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_tag_fixture() {
        let json = include_str!("../../tests/fixtures/tag.json");
        let tag: Tag = serde_json::from_str(json).unwrap();
        assert_eq!(tag.tag, "Open");
        assert_eq!(tag.meta.tag_type, Some(0));
        assert_eq!(tag.meta.num_items, Some(2));
    }
}
