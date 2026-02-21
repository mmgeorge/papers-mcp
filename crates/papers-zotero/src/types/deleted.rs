use serde::{Deserialize, Serialize};

/// Keys of objects deleted from a library since a given version.
///
/// Returned by `GET /users/<id>/deleted?since=<version>`. All fields are
/// arrays of string keys (or tag names). Empty arrays mean nothing of that
/// type was deleted in the requested range.
///
/// Use `last_modified_version` from the [`VersionedResponse`] wrapper
/// to update your local sync checkpoint.
///
/// [`VersionedResponse`]: crate::response::VersionedResponse
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletedObjects {
    /// Keys of deleted collections.
    pub collections: Vec<String>,

    /// Keys of deleted saved searches.
    pub searches: Vec<String>,

    /// Keys of deleted items (includes attachments, notes, annotations).
    pub items: Vec<String>,

    /// Names of deleted tags.
    pub tags: Vec<String>,

    /// Keys of deleted settings.
    pub settings: Vec<String>,
}
