use std::collections::HashMap;

// -- Enums --

#[derive(Default, Clone, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Markdown,
    Html,
    Json,
    Chunks,
}

#[derive(Default, Clone, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProcessingMode {
    Fast,
    #[default]
    Balanced,
    Accurate,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarkerStatus {
    Processing,
    Complete,
    Failed,
}

// -- Request --

/// Request parameters for the DataLab Marker conversion API.
///
/// Exactly one of `file` or `file_url` must be set.
pub struct MarkerRequest {
    /// Raw file bytes to upload. Required when `file_url` is not set.
    pub file: Option<Vec<u8>>,
    /// Filename for the uploaded file (e.g. `"paper.pdf"`). Used when `file` is set.
    pub filename: Option<String>,
    /// Public URL to the file. Alternative to `file`.
    pub file_url: Option<String>,
    /// Output format. Defaults to `Markdown`.
    pub output_format: OutputFormat,
    /// Processing mode. Defaults to `Balanced`.
    pub mode: ProcessingMode,
    /// Maximum number of pages to process.
    pub max_pages: Option<u32>,
    /// Page range (0-indexed). E.g. `"0-5"` or `"1,3,5"`.
    pub page_range: Option<String>,
    /// Insert page delimiters in output.
    pub paginate: bool,
    /// Force reprocessing even if cached.
    pub skip_cache: bool,
    /// Skip extracting images.
    pub disable_image_extraction: bool,
    /// Skip generating image captions.
    pub disable_image_captions: bool,
    /// Save intermediate checkpoint for downstream extraction steps.
    pub save_checkpoint: bool,
    /// HTML mode only: adds `data-block-id` attributes.
    pub add_block_ids: bool,
    /// Include markdown alongside chunks output.
    pub include_markdown_in_chunks: bool,
    /// Preserve spreadsheet table structure.
    pub keep_spreadsheet_formatting: bool,
    /// JSON schema for structured data extraction.
    pub page_schema: Option<serde_json::Value>,
    /// Schema for document segmentation.
    pub segmentation_schema: Option<String>,
    /// Extra Marker config (e.g. force_ocr, languages).
    pub additional_config: Option<serde_json::Value>,
    /// Comma-separated extras: `track_changes`, `chart_understanding`, etc.
    pub extras: Option<String>,
    /// Fence auto-generated captions.
    pub fence_synthetic_captions: bool,
    /// URL to POST results to when processing completes.
    pub webhook_url: Option<String>,
}

impl Default for MarkerRequest {
    fn default() -> Self {
        Self {
            file: None,
            filename: None,
            file_url: None,
            output_format: OutputFormat::default(),
            mode: ProcessingMode::default(),
            max_pages: None,
            page_range: None,
            paginate: false,
            skip_cache: false,
            disable_image_extraction: false,
            disable_image_captions: false,
            save_checkpoint: false,
            add_block_ids: false,
            include_markdown_in_chunks: false,
            keep_spreadsheet_formatting: false,
            page_schema: None,
            segmentation_schema: None,
            additional_config: None,
            extras: None,
            fence_synthetic_captions: false,
            webhook_url: None,
        }
    }
}

// -- Submit response --

/// Response from POST /api/v1/marker (submit).
#[derive(serde::Deserialize)]
pub struct MarkerSubmitResponse {
    pub success: bool,
    pub request_id: String,
    pub request_check_url: String,
}

// -- Poll response --

/// Response from GET /api/v1/marker/{request_id} (poll).
#[derive(serde::Deserialize)]
pub struct MarkerPollResponse {
    pub success: bool,
    pub status: MarkerStatus,
    pub output_format: Option<String>,
    pub markdown: Option<String>,
    pub html: Option<String>,
    pub json: Option<serde_json::Value>,
    pub chunks: Option<serde_json::Value>,
    pub extraction_schema_json: Option<String>,
    pub segmentation_results: Option<serde_json::Value>,
    pub images: Option<HashMap<String, String>>,
    pub metadata: Option<serde_json::Value>,
    pub error: Option<String>,
    pub error_in: Option<String>,
    pub page_count: Option<u32>,
    pub checkpoint_id: Option<String>,
    pub versions: Option<serde_json::Value>,
    pub parse_quality_score: Option<f64>,
    pub runtime: Option<f64>,
    pub cost_breakdown: Option<serde_json::Value>,
}

// -- Step types response --

/// A single workflow step type.
#[derive(serde::Deserialize)]
pub struct StepType {
    pub id: u32,
    #[serde(rename = "type")]
    pub type_: String,
    pub step_type: String,
    pub name: String,
    pub description: String,
    pub settings_schema: serde_json::Value,
    pub version: String,
    pub is_public: bool,
}

/// Response from GET /api/v1/workflows/step-types.
#[derive(serde::Deserialize)]
pub struct StepTypesResponse {
    pub step_types: Vec<StepType>,
}
