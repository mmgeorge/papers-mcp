use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::common::LinkEntry;

/// A Zotero group library.
///
/// Groups are shared libraries that multiple users can access. Each group has
/// its own items, collections, and tags. Group endpoints use
/// `/groups/<groupId>` as the path prefix instead of `/users/<userId>`.
///
/// # Example
///
/// ```json
/// {
///     "id": 12345,
///     "version": 50,
///     "links": { "self": { "href": "..." } },
///     "meta": { "created": "2024-01-01T00:00:00Z", "lastModified": "2024-06-01T00:00:00Z", "numItems": 100 },
///     "data": {
///         "id": 12345,
///         "version": 50,
///         "name": "Research Group",
///         "owner": 16916553,
///         "type": "PublicClosed",
///         "description": "Our shared research library",
///         "libraryEditing": "members",
///         "libraryReading": "all",
///         "fileEditing": "members"
///     }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: u64,
    pub version: u64,
    #[serde(default)]
    pub links: HashMap<String, LinkEntry>,
    #[serde(default)]
    pub meta: GroupMeta,
    pub data: GroupData,
}

/// Group metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupMeta {
    pub created: Option<String>,
    #[serde(rename = "lastModified")]
    pub last_modified: Option<String>,
    #[serde(rename = "numItems")]
    pub num_items: Option<u64>,
}

/// Group data payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupData {
    pub id: u64,
    pub version: u64,
    pub name: String,
    pub owner: Option<u64>,
    #[serde(rename = "type")]
    pub group_type: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "libraryEditing")]
    pub library_editing: Option<String>,
    #[serde(rename = "libraryReading")]
    pub library_reading: Option<String>,
    #[serde(rename = "fileEditing")]
    pub file_editing: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_group() {
        let json = r#"{
            "id": 12345,
            "version": 50,
            "links": {},
            "meta": { "created": "2024-01-01T00:00:00Z", "numItems": 100 },
            "data": {
                "id": 12345,
                "version": 50,
                "name": "Test Group",
                "owner": 16916553,
                "type": "PublicClosed",
                "description": "A test group",
                "libraryEditing": "members",
                "libraryReading": "all",
                "fileEditing": "members"
            }
        }"#;
        let group: Group = serde_json::from_str(json).unwrap();
        assert_eq!(group.id, 12345);
        assert_eq!(group.data.name, "Test Group");
        assert_eq!(group.data.group_type.as_deref(), Some("PublicClosed"));
        assert_eq!(group.meta.num_items, Some(100));
    }
}
