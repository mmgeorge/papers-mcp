use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::common::*;

/// A Zotero collection (folder for organizing items).
///
/// Collections form a tree: each collection has an optional `parentCollection`.
/// Items can belong to multiple collections.
///
/// # Example
///
/// ```json
/// {
///     "key": "QVK3WHF2",
///     "version": 3679,
///     "library": { "type": "user", "id": 16916553, "name": "mattmg", "links": {} },
///     "links": { "self": { "href": "..." }, "up": { "href": "..." } },
///     "meta": { "numCollections": 0, "numItems": 1 },
///     "data": {
///         "key": "QVK3WHF2",
///         "version": 3679,
///         "name": "Variational Calculus",
///         "parentCollection": "BXHE5XUF",
///         "relations": {}
///     }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub key: String,
    pub version: u64,
    pub library: Library,
    #[serde(default)]
    pub links: HashMap<String, LinkEntry>,
    #[serde(default)]
    pub meta: CollectionMeta,
    pub data: CollectionData,
}

/// Collection metadata returned in the `meta` object.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CollectionMeta {
    /// Number of direct sub-collections.
    #[serde(rename = "numCollections")]
    pub num_collections: Option<u64>,
    /// Number of items directly in this collection.
    #[serde(rename = "numItems")]
    pub num_items: Option<u64>,
}

/// Collection data payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionData {
    pub key: String,
    pub version: u64,
    pub name: String,
    /// Parent collection key, or `false` if top-level. The API returns the
    /// JSON literal `false` for top-level collections instead of `null`.
    #[serde(rename = "parentCollection")]
    pub parent_collection: serde_json::Value,
    pub relations: Option<serde_json::Value>,
}

impl CollectionData {
    /// Returns the parent collection key if this is a sub-collection,
    /// or `None` if top-level.
    pub fn parent_key(&self) -> Option<&str> {
        self.parent_collection.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_collection_fixture() {
        let json = include_str!("../../tests/fixtures/collection.json");
        let coll: Collection = serde_json::from_str(json).unwrap();
        assert_eq!(coll.key, "QVK3WHF2");
        assert_eq!(coll.data.name, "Variational Calculus");
        assert_eq!(coll.data.parent_key(), Some("BXHE5XUF"));
        assert_eq!(coll.meta.num_items, Some(1));
    }
}
