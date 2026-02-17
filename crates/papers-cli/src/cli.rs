use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "papers", about = "Query the OpenAlex academic research database", term_width = 100)]
pub struct Cli {
    #[command(subcommand)]
    pub entity: EntityCommand,
}

#[derive(Subcommand)]
pub enum EntityCommand {
    /// Scholarly works: articles, preprints, datasets, and more
    Work {
        #[command(subcommand)]
        cmd: WorkCommand,
    },
    /// Disambiguated researcher profiles
    Author {
        #[command(subcommand)]
        cmd: AuthorCommand,
    },
    /// Publishing venues: journals, repositories, conferences
    Source {
        #[command(subcommand)]
        cmd: SourceCommand,
    },
    /// Research organizations: universities, hospitals, companies
    Institution {
        #[command(subcommand)]
        cmd: InstitutionCommand,
    },
    /// Research topic hierarchy (domain → field → subfield → topic)
    Topic {
        #[command(subcommand)]
        cmd: TopicCommand,
    },
    /// Publishing organizations (e.g. Elsevier, Springer Nature)
    Publisher {
        #[command(subcommand)]
        cmd: PublisherCommand,
    },
    /// Grant-making organizations (e.g. NIH, NSF, ERC)
    Funder {
        #[command(subcommand)]
        cmd: FunderCommand,
    },
    /// Research domains (broadest level of topic hierarchy, 4 total)
    Domain {
        #[command(subcommand)]
        cmd: DomainCommand,
    },
    /// Academic fields (second level of topic hierarchy, 26 total)
    Field {
        #[command(subcommand)]
        cmd: FieldCommand,
    },
    /// Research subfields (third level of topic hierarchy, ~252 total)
    Subfield {
        #[command(subcommand)]
        cmd: SubfieldCommand,
    },
    /// Deprecated concept taxonomy (autocomplete only)
    Concept {
        #[command(subcommand)]
        cmd: ConceptCommand,
    },
}

/// Shared args for all list commands
#[derive(Args, Clone)]
pub struct ListArgs {
    /// Full-text search query
    #[arg(long, short = 's')]
    pub search: Option<String>,

    /// Filter expression (comma-separated AND conditions, pipe for OR)
    #[arg(long, short = 'f')]
    pub filter: Option<String>,

    /// Sort field with optional :desc (e.g. "cited_by_count:desc")
    #[arg(long)]
    pub sort: Option<String>,

    /// Results per page
    #[arg(long, short = 'n', default_value = "10")]
    pub per_page: u32,

    /// Page number for offset pagination
    #[arg(long)]
    pub page: Option<u32>,

    /// Cursor for cursor-based pagination (use "*" to start)
    #[arg(long)]
    pub cursor: Option<String>,

    /// Random sample of N results
    #[arg(long)]
    pub sample: Option<u32>,

    /// Seed for reproducible sampling
    #[arg(long)]
    pub seed: Option<u32>,

    /// Output raw JSON instead of formatted text
    #[arg(long)]
    pub json: bool,
}

/// Shorthand filter flags for `work list`.
///
/// These resolve to real OpenAlex filter expressions. ID-based filters accept
/// either an OpenAlex entity ID or a search string (resolved to the top result
/// by citation count).
#[derive(Args, Clone, Default)]
pub struct WorkFilterArgs {
    /// Filter by author name or OpenAlex author ID (e.g. "einstein", "Albert Einstein", or "A5108093963")
    #[arg(long)]
    pub author: Option<String>,

    /// Filter by topic name or OpenAlex topic ID (e.g. "deep learning",
    /// "computer graphics and visualization techniques", "advanced numerical analysis techniques",
    /// or "T10320"). Run `papers topic list -s <query>` to browse topics.
    #[arg(long)]
    pub topic: Option<String>,

    /// Filter by domain name or ID. The 4 domains: 1 Life Sciences, 2 Social Sciences,
    /// 3 Physical Sciences, 4 Health Sciences (e.g. "physical sciences" or "3")
    #[arg(long)]
    pub domain: Option<String>,

    /// Filter by field name or ID (e.g. "computer science", "engineering", "mathematics", or "17").
    /// Run `papers field list` to browse all 26 fields.
    #[arg(long)]
    pub field: Option<String>,

    /// Filter by subfield name or ID (e.g. "artificial intelligence", "computer graphics",
    /// "computational geometry", or "1702"). Run `papers subfield list -s <query>` or
    /// `papers subfield autocomplete <query>` to discover subfields.
    #[arg(long)]
    pub subfield: Option<String>,

    /// Filter by publisher name or ID. Supports pipe-separated OR (e.g. "acm", "acm|ieee", "P4310319798")
    #[arg(long)]
    pub publisher: Option<String>,

    /// Filter by source (journal/conference) name or ID (e.g. "siggraph", "nature", or "S131921510")
    #[arg(long)]
    pub source: Option<String>,

    /// Filter by publication year (e.g. "2024", ">2008", "2008-2024", "2020|2021")
    #[arg(long)]
    pub year: Option<String>,

    /// Filter by citation count (e.g. ">100", "10-50")
    #[arg(long)]
    pub citations: Option<String>,
}

#[derive(Subcommand)]
pub enum WorkCommand {
    /// List works with optional search/filter/sort
    #[command(after_help = "Advanced filtering: https://docs.openalex.org/api-entities/works/filter-works")]
    List {
        #[command(flatten)]
        args: ListArgs,
        #[command(flatten)]
        work_filters: WorkFilterArgs,
    },
    /// Get a single work by ID (OpenAlex ID, DOI, PMID, or PMCID)
    Get {
        /// Work ID
        id: String,
        /// Output raw JSON
        #[arg(long)]
        json: bool,
    },
    /// Type-ahead search for works by title
    Autocomplete {
        /// Search query
        query: String,
        /// Output raw JSON
        #[arg(long)]
        json: bool,
    },
    /// AI semantic search for similar works (requires OPENALEX_KEY)
    Find {
        /// Text to find similar works for
        query: String,
        /// Number of results (1-100)
        #[arg(long, short = 'n')]
        count: Option<u32>,
        /// Filter expression (https://docs.openalex.org/api-entities/works/filter-works)
        #[arg(long, short = 'f')]
        filter: Option<String>,
        /// Output raw JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum AuthorCommand {
    /// List authors with optional search/filter/sort
    #[command(after_help = "Advanced filtering: https://docs.openalex.org/api-entities/authors/filter-authors")]
    List {
        #[command(flatten)]
        args: ListArgs,
    },
    /// Get a single author by ID (OpenAlex ID or ORCID)
    Get {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Type-ahead search for authors
    Autocomplete {
        query: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum SourceCommand {
    /// List sources with optional search/filter/sort
    #[command(after_help = "Advanced filtering: https://docs.openalex.org/api-entities/sources/filter-sources")]
    List {
        #[command(flatten)]
        args: ListArgs,
    },
    /// Get a single source by ID (OpenAlex ID or ISSN)
    Get {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Type-ahead search for sources
    Autocomplete {
        query: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum InstitutionCommand {
    /// List institutions with optional search/filter/sort
    #[command(after_help = "Advanced filtering: https://docs.openalex.org/api-entities/institutions/filter-institutions")]
    List {
        #[command(flatten)]
        args: ListArgs,
    },
    /// Get a single institution by ID (OpenAlex ID or ROR)
    Get {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Type-ahead search for institutions
    Autocomplete {
        query: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum TopicCommand {
    /// List topics with optional search/filter/sort
    #[command(after_help = "Advanced filtering: https://docs.openalex.org/api-entities/topics/filter-topics")]
    List {
        #[command(flatten)]
        args: ListArgs,
    },
    /// Get a single topic by OpenAlex ID
    Get {
        id: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum PublisherCommand {
    /// List publishers with optional search/filter/sort
    #[command(after_help = "Advanced filtering: https://docs.openalex.org/api-entities/publishers/filter-publishers")]
    List {
        #[command(flatten)]
        args: ListArgs,
    },
    /// Get a single publisher by OpenAlex ID
    Get {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Type-ahead search for publishers
    Autocomplete {
        query: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum FunderCommand {
    /// List funders with optional search/filter/sort
    #[command(after_help = "Advanced filtering: https://docs.openalex.org/api-entities/funders/filter-funders")]
    List {
        #[command(flatten)]
        args: ListArgs,
    },
    /// Get a single funder by OpenAlex ID
    Get {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Type-ahead search for funders
    Autocomplete {
        query: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum DomainCommand {
    /// List domains with optional search/filter/sort
    #[command(after_help = "Example filters: works_count:>100000000, display_name.search:physical\nFilter docs: https://docs.openalex.org/how-to-use-the-api/get-lists-of-entities/filter-entity-lists")]
    List {
        #[command(flatten)]
        args: ListArgs,
    },
    /// Get a single domain by numeric ID (1-4)
    Get {
        id: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum FieldCommand {
    /// List fields with optional search/filter/sort
    #[command(after_help = "Example filters: domain.id:domains/3, works_count:>1000000\nFilter docs: https://docs.openalex.org/how-to-use-the-api/get-lists-of-entities/filter-entity-lists")]
    List {
        #[command(flatten)]
        args: ListArgs,
    },
    /// Get a single field by numeric ID (e.g. 17)
    Get {
        id: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum SubfieldCommand {
    /// List subfields with optional search/filter/sort
    #[command(after_help = "Example filters: field.id:fields/17, works_count:>100000\nFilter docs: https://docs.openalex.org/how-to-use-the-api/get-lists-of-entities/filter-entity-lists")]
    List {
        #[command(flatten)]
        args: ListArgs,
    },
    /// Get a single subfield by numeric ID (e.g. 1702)
    Get {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Type-ahead search for subfields
    Autocomplete {
        query: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum ConceptCommand {
    /// Type-ahead search for concepts (deprecated taxonomy)
    Autocomplete {
        query: String,
        #[arg(long)]
        json: bool,
    },
}
