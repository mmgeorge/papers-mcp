/// Errors returned by [`ZoteroClient`](crate::ZoteroClient) methods.
///
/// # Variants
///
/// - [`Http`](ZoteroError::Http) — network or connection failure (wraps
///   [`reqwest::Error`])
/// - [`Json`](ZoteroError::Json) — response body could not be deserialized
///   (wraps [`serde_json::Error`])
/// - [`Api`](ZoteroError::Api) — the Zotero API returned a non-success HTTP
///   status code (e.g. 404 for unknown item, 403 for forbidden, 500 for server
///   error)
#[derive(Debug, thiserror::Error)]
pub enum ZoteroError {
    /// Network or connection error from reqwest.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Failed to deserialize the JSON response body.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// The API returned a non-success HTTP status code.
    ///
    /// `status` is the HTTP status code (e.g. 404, 403, 500) and `message`
    /// contains the response body text.
    #[error("API error (status {status}): {message}")]
    Api { status: u16, message: String },
}

/// A [`Result`](std::result::Result) alias with [`ZoteroError`] as the error
/// type.
pub type Result<T> = std::result::Result<T, ZoteroError>;
