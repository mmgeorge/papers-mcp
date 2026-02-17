use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::common::*;

/// A Zotero saved search.
///
/// Saved searches store a set of conditions that can be used to dynamically
/// find items matching specific criteria.
///
/// # Example
///
/// ```json
/// {
///     "key": "ABCD1234",
///     "version": 100,
///     "library": { "type": "user", "id": 16916553, "name": "mattmg", "links": {} },
///     "links": { "self": { "href": "..." } },
///     "data": {
///         "key": "ABCD1234",
///         "version": 100,
///         "name": "Recent ML papers",
///         "conditions": [
///             { "condition": "tag", "operator": "is", "value": "ML" },
///             { "condition": "dateAdded", "operator": "isAfter", "value": "2024-01-01" }
///         ]
///     }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSearch {
    pub key: String,
    pub version: u64,
    pub library: Library,
    #[serde(default)]
    pub links: HashMap<String, LinkEntry>,
    pub data: SearchData,
}

/// Saved search data payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchData {
    pub key: String,
    pub version: u64,
    pub name: String,
    #[serde(default)]
    pub conditions: Vec<SearchCondition>,
}

/// A single condition in a saved search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchCondition {
    pub condition: String,
    pub operator: String,
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_search() {
        let json = r#"{
            "key": "ABCD1234",
            "version": 100,
            "library": { "type": "user", "id": 1, "name": "test", "links": {} },
            "links": {},
            "data": {
                "key": "ABCD1234",
                "version": 100,
                "name": "Test Search",
                "conditions": [
                    { "condition": "tag", "operator": "is", "value": "ML" }
                ]
            }
        }"#;
        let search: SavedSearch = serde_json::from_str(json).unwrap();
        assert_eq!(search.key, "ABCD1234");
        assert_eq!(search.data.name, "Test Search");
        assert_eq!(search.data.conditions.len(), 1);
        assert_eq!(search.data.conditions[0].condition, "tag");
    }
}
