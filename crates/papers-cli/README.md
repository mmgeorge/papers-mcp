# papers-cli

[![crates.io](https://img.shields.io/crates/v/papers-cli.svg)](https://crates.io/crates/papers-cli)

A command-line interface for the [OpenAlex](https://openalex.org) academic research database.
Query 240M+ scholarly works, authors, journals, institutions, topics, publishers, and funders.

## Install

```sh
cargo install --path crates/papers-cli
```

## Usage

```
Query the OpenAlex academic research database

Usage: papers <COMMAND>

Commands:
  work         Scholarly works: articles, preprints, datasets, and more
  author       Disambiguated researcher profiles
  source       Publishing venues: journals, repositories, conferences
  institution  Research organizations: universities, hospitals, companies
  topic        Research topic hierarchy (domain → field → subfield → topic)
  publisher    Publishing organizations (e.g. Elsevier, Springer Nature)
  funder       Grant-making organizations (e.g. NIH, NSF, ERC)
  domain       Research domains (broadest level of topic hierarchy, 4 total)
  field        Academic fields (second level of topic hierarchy, 26 total)
  subfield     Research subfields (third level of topic hierarchy, ~252 total)
  zotero       Your personal Zotero reference library
  help         Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

Every command accepts `--json` to output raw JSON instead of formatted text.

### Listing

```
List works with optional search/filter/sort

Usage: papers work list [OPTIONS]

Options:
  -s, --search <SEARCH>            Full-text search query
  -f, --filter <FILTER>            Filter expression (comma-separated AND conditions, pipe for OR)
      --sort <SORT>                Sort field with optional :desc (e.g. "cited_by_count:desc")
  -n, --per-page <PER_PAGE>        Results per page [default: 10]
      --page <PAGE>                Page number for offset pagination
      --cursor <CURSOR>            Cursor for cursor-based pagination (use "*" to start)
      --sample <SAMPLE>            Random sample of N results
      --seed <SEED>                Seed for reproducible sampling
      --json                       Output raw JSON instead of formatted text
      --author <AUTHOR>            Filter by author name or OpenAlex author ID
      --topic <TOPIC>              Filter by topic name or OpenAlex topic ID
      --domain <DOMAIN>            Filter by domain name or ID
      --field <FIELD>              Filter by field name or ID
      --subfield <SUBFIELD>        Filter by subfield name or ID
      --publisher <PUBLISHER>      Filter by publisher name or ID
      --source <SOURCE>            Filter by source (journal/conference) name or ID
      --institution <INSTITUTION>  Filter by institution name or ID
      --year <YEAR>                Filter by publication year (e.g. "2024", ">2008", "2008-2024")
      --citations <CITATIONS>      Filter by citation count (e.g. ">100", "10-50")
      --country <COUNTRY>          Filter by country code of author institutions (e.g. "US", "GB")
      --continent <CONTINENT>      Filter by continent of author institutions (e.g. "europe", "asia")
      --type <ENTITY_TYPE>         Filter by work type (e.g. "article", "preprint", "dataset")
      --open                       Filter for open access works only
  -h, --help                       Print help

Advanced filtering: https://docs.openalex.org/api-entities/works/filter-works
```

In addition to the base options above, each entity has **filter aliases** — shorthand flags
that resolve to OpenAlex filter expressions so you don't need to remember the raw syntax.
See [Filter aliases](#filter-aliases) below.

### Getting

The `get` subcommand accepts **any of these as its argument**:

- OpenAlex IDs: `W2741809807`, `A5028826050`, `https://openalex.org/W2741809807`
- DOIs: `https://doi.org/10.7717/peerj.4375`, `10.7717/peerj.4375`
- ORCIDs: `https://orcid.org/0000-0002-1825-0097`
- ROR IDs: `https://ror.org/03vek6s52`
- PubMed IDs: `pmid:12345678`
- ISSNs: `0028-0836` (for sources)
- Hierarchy IDs: `3` (domains), `17` (fields), `1702` (subfields)
- **Search queries**: any other string is resolved to the top result by citation count

```
$ papers work get "attention is all you need"
$ papers author get "yoshua bengio"
$ papers source get "nature"
$ papers institution get "MIT"
```

## Examples

### Search for works

```
$ papers work list -s "attention is all you need" -n 3
```

```
Found 1556581 results · page 1 (showing 3)

  1  Attention Is All You Need (2025)
     Ashish Vaswani · Noam Shazeer · Niki Parmar · Jakob Uszkoreit · Llion Jones · Aidan N. Gomez · Łukasz Kaiser · Illia Polosukhin
     preprint · 6488 citations · OA: Yes
     Topic: Natural Language Processing Techniques
     DOI: https://doi.org/10.65215/2q58a426

     The dominant sequence transduction models are based on complex recurrent or convolutional neural
     networks in an encoder-decoder configuration...

  2  Attention Is All You Need In Speech Separation (2021)
     Cem Subakan · Mirco Ravanelli · Samuele Cornell · Mirko Bronzi · Jianyuan Zhong
     article · 574 citations · OA: No
     Topic: Speech and Audio Processing
     DOI: https://doi.org/10.1109/icassp39728.2021.9413901
     ...
```

### Filter with aliases

Instead of raw filter expressions, use shorthand flags:

```
$ papers work list --author "Yann LeCun" --year 2020-2024 --open -n 3
$ papers work list --topic "deep learning" --citations ">100" --sort cited_by_count:desc
$ papers work list --institution MIT --field "computer science" --year 2024
$ papers author list --institution harvard --country US --h-index ">50"
$ papers source list --publisher springer --type journal
```

Aliases can be combined with each other and with `--search`/`--filter`/`--sort`:

```
$ papers work list -s "transformer" --year 2024 --open --sort cited_by_count:desc -n 5
```

### Filter with raw expressions

You can still use raw [OpenAlex filter syntax](https://docs.openalex.org/how-to-use-the-api/get-lists-of-entities/filter-entity-lists) via `--filter`:

```
$ papers work list -f "publication_year:2024,is_oa:true" --sort cited_by_count:desc -n 3
```

```
Found 6211989 results · page 1 (showing 3)

  1  Global cancer statistics 2022: GLOBOCAN estimates... (2024)
     Freddie Bray · Mathieu Laversanne · ...
     CA A Cancer Journal for Clinicians · article · 19449 citations · OA: Yes
     Topic: Global Cancer Incidence and Screening
     DOI: https://doi.org/10.3322/caac.21834
     ...
```

### Get a single work

```
$ papers work get W2741809807
```

```
Work: The state of OA: a large-scale analysis of the prevalence and impact of Open Access articles
ID:   https://openalex.org/W2741809807
DOI:  https://doi.org/10.7717/peerj.4375
Year: 2018 · Type: book-chapter
OA:   Yes (gold) · https://doi.org/10.7717/peerj.4375
Citations: 1149
Topic: scientometrics and bibliometrics research (Statistics, Probability and Uncertainty → Decision Sciences → Social Sciences)

Authors:
   1. Heather Piwowar (first)  Impact Technology Development (United States)
   2. Jason Priem (middle)  Impact Technology Development (United States)
   3. Vincent Larivière (middle)  Université de Montréal
   ...

Abstract:
  Despite growing interest in Open Access (OA) to scholarly literature, there is an unmet need for
  large-scale, up-to-date, and reproducible studies assessing the prevalence and characteristics of OA...
```

You can also look up works by DOI or search query:

```
$ papers work get https://doi.org/10.7717/peerj.4375
$ papers work get "state of open access"
```

### Author autocomplete

```
$ papers author autocomplete "yoshua bengio"
```

```
 1  Yoshua Bengio [authors/A5028826050]
    Mila - Quebec Artificial Intelligence Institute, Canada
    1270 citations
 2  Yoshua Bengio [authors/A5125732408]
    Mila - Quebec Artificial Intelligence Institute, Canada
    0 citations
...
```

### List journals

```
$ papers source list -s "nature" -n 3
```

```
Found 278 results · page 1 (showing 3)

  1  Nature
     ISSN: 0028-0836 · journal · OA: No · h-index: 1822
     Publisher: Springer Nature

  2  Nature Communications
     ISSN: 2041-1723 · journal · OA: Yes · h-index: 719
     Publisher: Springer Nature

  3  Nature Genetics
     ISSN: 1061-4036 · journal · OA: No · h-index: 776
     Publisher: Springer Nature
```

### JSON output

Append `--json` to any command to get machine-readable output:

```
$ papers work get W2741809807 --json
```

```json
{
  "id": "https://openalex.org/W2741809807",
  "doi": "https://doi.org/10.7717/peerj.4375",
  "title": "The state of OA: a large-scale analysis...",
  "publication_year": 2018,
  "type": "book-chapter",
  "cited_by_count": 1149,
  "referenced_works": [...],
  ...
}
```

List responses in `--json` mode return a slim subset of fields (no `referenced_works`,
`counts_by_year`, etc.) for conciseness. Use `work get --json` to retrieve the full record.

### Semantic search (requires API key)

```
$ OPENALEX_KEY=<your-key> papers work find "transformer attention mechanism self-supervised learning" -n 5
```

Requires a [polite pool API key](https://docs.openalex.org/how-to-use-the-api/rate-limits-and-authentication)
with semantic search credits enabled.

## Filter aliases

Shorthand flags that resolve to OpenAlex filter expressions. Entity-based aliases (like `--author`)
accept either an OpenAlex ID or a search string that gets resolved to the top result by citation count.

### `work list`

| Flag | Example | Resolves to |
|------|---------|-------------|
| `--author` | `"einstein"`, `A5108093963` | `authorships.author.id:<id>` |
| `--topic` | `"deep learning"`, `T10320` | `topics.id:<id>` |
| `--domain` | `"physical sciences"`, `3` | `topics.domain.id:<id>` |
| `--field` | `"computer science"`, `17` | `topics.field.id:<id>` |
| `--subfield` | `"artificial intelligence"`, `1702` | `topics.subfield.id:<id>` |
| `--publisher` | `"acm"`, `"acm\|ieee"` | `primary_location.source.publisher_lineage:<id>` |
| `--source` | `"nature"`, `S137773608` | `primary_location.source.id:<id>` |
| `--institution` | `"mit"`, `I136199984` | `authorships.institutions.lineage:<id>` |
| `--year` | `2024`, `>2008`, `2008-2024` | `publication_year:<value>` |
| `--citations` | `">100"`, `"10-50"` | `cited_by_count:<value>` |
| `--country` | `US`, `GB` | `authorships.countries:<value>` |
| `--continent` | `europe`, `asia` | `authorships.continents:<value>` |
| `--type` | `article`, `preprint` | `type:<value>` |
| `--open` | *(flag)* | `is_oa:true` |

### `author list`

| Flag | Example |
|------|---------|
| `--institution` | `"harvard"`, `"mit"`, `I136199984` |
| `--country` | `US`, `GB` |
| `--continent` | `europe`, `asia` |
| `--citations` | `">1000"`, `"100-500"` |
| `--works` | `">500"`, `"100-200"` |
| `--h-index` | `">50"`, `"10-20"` |

### `source list`

| Flag | Example |
|------|---------|
| `--publisher` | `"springer"`, `P4310319798` |
| `--country` | `US`, `GB` |
| `--continent` | `europe` |
| `--type` | `journal`, `repository`, `conference` |
| `--open` | *(flag)* |
| `--citations` | `">10000"` |
| `--works` | `">100000"` |

### `institution list`

| Flag | Example |
|------|---------|
| `--country` | `US`, `GB` |
| `--continent` | `europe`, `asia` |
| `--type` | `education`, `healthcare`, `company` |
| `--citations` | `">100000"` |
| `--works` | `">100000"` |

### `topic list`

| Flag | Example |
|------|---------|
| `--domain` | `"life sciences"`, `3` |
| `--field` | `"computer science"`, `17` |
| `--subfield` | `"artificial intelligence"`, `1702` |
| `--citations` | `">1000"` |
| `--works` | `">1000"` |

### Other entities

`publisher list`, `funder list`, `field list`, `subfield list`, and `domain list` also support
relevant subsets of `--country`, `--continent`, `--domain`, `--field`, `--citations`, and `--works`.

## Zotero personal library

Access your [Zotero](https://www.zotero.org) reference library from the command line.

**Setup** — set two environment variables:

```sh
export ZOTERO_USER_ID=<your-user-id>   # from zotero.org/settings/keys
export ZOTERO_API_KEY=<your-api-key>   # from zotero.org/settings/keys
```

### Entities and commands

| Entity | Commands |
|--------|----------|
| `work` | `list`, `get`, `collections`, `notes`, `attachments`, `annotations`, `tags` |
| `attachment` | `list`, `get`, `file` |
| `annotation` | `list`, `get` |
| `note` | `list`, `get` |
| `collection` | `list`, `get`, `works`, `attachments`, `notes`, `annotations`, `subcollections`, `tags` |
| `tag` | `list`, `get` |
| `search` | `list`, `get` |
| `group` | `list` |

All commands accept `--json` to output raw JSON.

### Examples

```sh
# List your starred papers (sorted by date modified)
papers zotero work list --tag Starred --sort dateModified --direction desc

# Search for rendering papers
papers zotero work list --search "rendering" --type conferencePaper -n 5

# Get a single work
papers zotero work get <key> --json

# List collections a work belongs to
papers zotero work collections <key>

# List annotations on all PDFs of a work
papers zotero work annotations <key>

# List all attachments
papers zotero attachment list --limit 20

# Download a PDF attachment
papers zotero attachment file <attachment-key> --output paper.pdf

# Browse collections
papers zotero collection list --top
papers zotero collection works <collection-key> --type conferencePaper

# Search tags
papers zotero tag list --search "Star" --qmode startsWith
papers zotero tag list --top

# Saved searches and groups
papers zotero search list
papers zotero group list
```

## Raw filter syntax

For cases not covered by aliases, use `-f`/`--filter` with the
[OpenAlex filter syntax](https://docs.openalex.org/how-to-use-the-api/get-lists-of-entities/filter-entity-lists):

| Example | Meaning |
|---------|---------|
| `publication_year:2024` | Published in 2024 |
| `is_oa:true` | Open access only |
| `publication_year:2020\|2021\|2022` | Published 2020, 2021, or 2022 |
| `authorships.author.id:A5028826050` | Works by a specific author |
| `primary_location.source.id:S137773608` | Works in a specific journal |
| `cited_by_count:>100` | More than 100 citations |

Combine filters with commas (AND): `-f "publication_year:2024,is_oa:true"`
