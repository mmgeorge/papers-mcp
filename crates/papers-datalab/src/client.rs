use std::time::Duration;

use crate::error::{DatalabError, Result};
use crate::types::{MarkerPollResponse, MarkerRequest, MarkerStatus, MarkerSubmitResponse, StepTypesResponse};

const DEFAULT_BASE_URL: &str = "https://www.datalab.to";

/// Async client for the DataLab Marker REST API.
///
/// # Authentication
///
/// All requests require an API key sent via the `X-API-Key` header.
/// Create the client with [`DatalabClient::new`] or load from the
/// `DATALAB_API_KEY` environment variable with [`DatalabClient::from_env`].
///
/// # Usage
///
/// ```no_run
/// # async fn example() -> papers_datalab::Result<()> {
/// use papers_datalab::{DatalabClient, MarkerRequest, OutputFormat, ProcessingMode};
///
/// let client = DatalabClient::from_env()?;
/// let pdf_bytes = std::fs::read("paper.pdf").unwrap();
///
/// let result = client.convert_document(MarkerRequest {
///     file: Some(pdf_bytes),
///     filename: Some("paper.pdf".into()),
///     output_format: vec![OutputFormat::Markdown],
///     mode: ProcessingMode::Accurate,
///     ..Default::default()
/// }).await?;
///
/// println!("{}", result.markdown.unwrap_or_default());
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct DatalabClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl DatalabClient {
    /// Create a new client with an explicit API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }

    /// Create a client from the `DATALAB_API_KEY` environment variable.
    ///
    /// Returns [`DatalabError::MissingApiKey`] if the variable is not set.
    pub fn from_env() -> Result<Self> {
        let key = std::env::var("DATALAB_API_KEY").map_err(|_| DatalabError::MissingApiKey)?;
        Ok(Self::new(key))
    }

    /// High-level: submit a document and poll until conversion is complete.
    ///
    /// Uses a 2-second poll interval. Returns the completed [`MarkerPollResponse`]
    /// or an error if the job fails. No timeout is applied — the caller is
    /// responsible for cancellation if needed.
    pub async fn convert_document(&self, req: MarkerRequest) -> Result<MarkerPollResponse> {
        let submit = self.submit_marker(req).await?;
        let request_id = submit.request_id;

        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let poll = self.get_marker_result(&request_id).await?;
            match poll.status {
                MarkerStatus::Complete => return Ok(poll),
                MarkerStatus::Failed => {
                    return Err(DatalabError::Processing(
                        poll.error.unwrap_or_else(|| "unknown processing error".to_string()),
                    ));
                }
                MarkerStatus::Processing => continue,
            }
        }
    }

    /// POST /api/v1/marker — submit a conversion job.
    ///
    /// Returns immediately with a `request_id`. Use [`get_marker_result`](Self::get_marker_result)
    /// to poll for the result, or call [`convert_document`](Self::convert_document) to do both.
    pub async fn submit_marker(&self, req: MarkerRequest) -> Result<MarkerSubmitResponse> {
        // Validate: exactly one of file or file_url must be provided
        if req.file.is_none() && req.file_url.is_none() {
            return Err(DatalabError::InvalidRequest);
        }

        let mut form = reqwest::multipart::Form::new();

        // File source
        if let Some(bytes) = req.file {
            let filename = req.filename.unwrap_or_else(|| "document.pdf".to_string());
            let part = reqwest::multipart::Part::bytes(bytes)
                .file_name(filename)
                .mime_str("application/pdf")
                .map_err(|e| DatalabError::Http(e))?;
            form = form.part("file", part);
        } else if let Some(url) = req.file_url {
            form = form.text("file_url", url);
        }

        // Output format (serialize to comma-joined string)
        let fmt = req.output_format.iter().map(|f| match f {
            crate::types::OutputFormat::Markdown => "markdown",
            crate::types::OutputFormat::Html => "html",
            crate::types::OutputFormat::Json => "json",
            crate::types::OutputFormat::Chunks => "chunks",
        }).collect::<Vec<_>>().join(",");
        form = form.text("output_format", fmt);

        // Processing mode
        let mode = match req.mode {
            crate::types::ProcessingMode::Fast => "fast",
            crate::types::ProcessingMode::Balanced => "balanced",
            crate::types::ProcessingMode::Accurate => "accurate",
        };
        form = form.text("mode", mode);

        // Optional scalar fields
        if let Some(max_pages) = req.max_pages {
            form = form.text("max_pages", max_pages.to_string());
        }
        if let Some(page_range) = req.page_range {
            form = form.text("page_range", page_range);
        }
        if req.paginate {
            form = form.text("paginate", "true");
        }
        if req.skip_cache {
            form = form.text("skip_cache", "true");
        }
        if req.disable_image_extraction {
            form = form.text("disable_image_extraction", "true");
        }
        if req.disable_image_captions {
            form = form.text("disable_image_captions", "true");
        }
        if req.save_checkpoint {
            form = form.text("save_checkpoint", "true");
        }
        if req.add_block_ids {
            form = form.text("add_block_ids", "true");
        }
        if req.include_markdown_in_chunks {
            form = form.text("include_markdown_in_chunks", "true");
        }
        if req.keep_spreadsheet_formatting {
            form = form.text("keep_spreadsheet_formatting", "true");
        }
        if req.fence_synthetic_captions {
            form = form.text("fence_synthetic_captions", "true");
        }
        if let Some(schema) = req.page_schema {
            form = form.text("page_schema", schema.to_string());
        }
        if let Some(seg_schema) = req.segmentation_schema {
            form = form.text("segmentation_schema", seg_schema);
        }
        if let Some(config) = req.additional_config {
            form = form.text("additional_config", config.to_string());
        }
        if let Some(extras) = req.extras {
            form = form.text("extras", extras);
        }
        if let Some(webhook) = req.webhook_url {
            form = form.text("webhook_url", webhook);
        }

        let url = format!("{}/api/v1/marker", self.base_url);
        let resp = self
            .http
            .post(&url)
            .header("X-API-Key", &self.api_key)
            .multipart(form)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(DatalabError::Api {
                status: status.as_u16(),
                message,
            });
        }

        let body = resp.text().await?;
        serde_json::from_str::<MarkerSubmitResponse>(&body)
            .map_err(|e| DatalabError::Api { status: 0, message: format!("JSON parse error: {e}") })
    }

    /// GET /api/v1/marker/{request_id} — poll for a single conversion result.
    ///
    /// Returns the current state of the job. `status` will be `processing`,
    /// `complete`, or `failed`. Poll every 2 seconds until `complete` or `failed`.
    pub async fn get_marker_result(&self, request_id: &str) -> Result<MarkerPollResponse> {
        let url = format!("{}/api/v1/marker/{}", self.base_url, request_id);
        let resp = self
            .http
            .get(&url)
            .header("X-API-Key", &self.api_key)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(DatalabError::Api {
                status: status.as_u16(),
                message,
            });
        }

        let body = resp.text().await?;
        serde_json::from_str::<MarkerPollResponse>(&body)
            .map_err(|e| DatalabError::Api { status: 0, message: format!("JSON parse error: {e}") })
    }

    /// GET /api/v1/workflows/step-types — list available workflow step types.
    pub async fn list_step_types(&self) -> Result<StepTypesResponse> {
        let url = format!("{}/api/v1/workflows/step-types", self.base_url);
        let resp = self
            .http
            .get(&url)
            .header("X-API-Key", &self.api_key)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(DatalabError::Api {
                status: status.as_u16(),
                message,
            });
        }

        Ok(resp.json::<StepTypesResponse>().await?)
    }
}
