use serde::{Deserialize, Serialize};

/// A single Zotero user setting entry.
///
/// Returned by `GET /users/<id>/settings/<key>` or as each value in the map
/// from `GET /users/<id>/settings`.
///
/// The `value` type varies by setting key:
/// - `tagColors` → array of `{name: String, color: String}` objects
/// - `lastPageIndex_u_<itemKey>` → integer page number
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingEntry {
    /// The setting value. Type varies by setting key.
    pub value: serde_json::Value,

    /// Library version when this setting was last modified.
    pub version: u64,
}
