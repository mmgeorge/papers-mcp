/// Errors returned by [`DatalabClient`](crate::DatalabClient) methods.
#[derive(thiserror::Error, Debug)]
pub enum DatalabError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },

    #[error("Processing failed: {0}")]
    Processing(String),

    #[error("DATALAB_API_KEY environment variable not set")]
    MissingApiKey,

    #[error("MarkerRequest must specify either `file` or `file_url`")]
    InvalidRequest,
}

/// A [`Result`](std::result::Result) alias with [`DatalabError`] as the error type.
pub type Result<T> = std::result::Result<T, DatalabError>;
