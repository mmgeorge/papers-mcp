//! Async Rust client for the [DataLab Marker REST API](https://www.datalab.to).
//!
//! DataLab Marker converts PDF and other documents to markdown, HTML, JSON,
//! or structured chunks using a cloud-based ML pipeline. Conversion is async:
//! submit a job with [`DatalabClient::submit_marker`] and poll for the result
//! with [`DatalabClient::get_marker_result`], or use the convenience method
//! [`DatalabClient::convert_document`] which handles polling automatically.
//!
//! # Quick start
//!
//! ```no_run
//! # async fn example() -> papers_datalab::Result<()> {
//! use papers_datalab::{DatalabClient, MarkerRequest, OutputFormat, ProcessingMode};
//!
//! let client = DatalabClient::from_env()?;
//! let pdf_bytes = std::fs::read("paper.pdf").unwrap();
//!
//! let result = client.convert_document(MarkerRequest {
//!     file: Some(pdf_bytes),
//!     filename: Some("paper.pdf".into()),
//!     output_format: vec![OutputFormat::Markdown],
//!     mode: ProcessingMode::Accurate,
//!     ..Default::default()
//! }).await?;
//!
//! println!("{}", result.markdown.unwrap_or_default());
//! # Ok(())
//! # }
//! ```
//!
//! # Authentication
//!
//! Set the `DATALAB_API_KEY` environment variable, or pass the key directly
//! to [`DatalabClient::new`].

pub mod client;
pub mod error;
pub mod types;

pub use client::DatalabClient;
pub use error::{DatalabError, Result};
pub use types::{
    MarkerPollResponse, MarkerRequest, MarkerStatus, MarkerSubmitResponse, OutputFormat,
    ProcessingMode, StepType, StepTypesResponse,
};
