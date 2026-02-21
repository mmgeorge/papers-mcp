use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Response from a Zotero write request (POST /items, POST /collections, POST /searches).
///
/// The API processes each object in the request array independently. Successful
/// creations/updates, unchanged objects (version matched), and failures are
/// reported separately. The map keys are the **string index** of the
/// corresponding input object (e.g. `"0"`, `"1"`, `"2"`).
///
/// # Example
///
/// ```json
/// {
///   "successful": {
///     "0": { "key": "ABCD1234", "version": 5, "data": { "itemType": "note", ... } }
///   },
///   "unchanged": {},
///   "failed": {}
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteResponse {
    /// Successfully created or updated objects, keyed by input array index.
    /// Values are the full saved objects returned by the server.
    #[serde(default)]
    pub successful: HashMap<String, serde_json::Value>,

    /// Objects that were unchanged (version matched, no-op), keyed by index.
    /// Values are the item keys.
    #[serde(default)]
    pub unchanged: HashMap<String, String>,

    /// Objects that failed to save, keyed by input array index.
    #[serde(default)]
    pub failed: HashMap<String, WriteFailed>,
}

impl WriteResponse {
    /// Returns `true` if all submitted objects were either saved successfully
    /// or were already up to date.
    pub fn is_ok(&self) -> bool {
        self.failed.is_empty()
    }

    /// Collect the keys of all successfully created/updated objects.
    pub fn successful_keys(&self) -> Vec<String> {
        self.successful
            .values()
            .filter_map(|v| v.get("key").and_then(|k| k.as_str()).map(|s| s.to_string()))
            .collect()
    }
}

/// A single failed write operation within a [`WriteResponse`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteFailed {
    pub key: Option<String>,
    pub code: u16,
    pub message: String,
}
