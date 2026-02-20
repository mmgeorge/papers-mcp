## [0.2.0] - 2026-02-20

### Features

- Add papers-zotero crate for Zotero Web API integration
- Add full-text PDF extraction tool
- Implement interactive pdf retrieval and zotero polling
- Integrate DataLab Marker API for PDF extraction
- Add zotero api
- Allow zotero key resolution by title or name
- Simplify zotero search and tag filtering
- Improved zotero integration. check for running

### Other

- Remove autoupdate

### Refactor

- Replace pdfium-render with pdf-extract

### Documentation

- Add crates.io badges to readmes
- Update and clean up readme files
- Add README for papers-zotero crate
- Add mit license badge and reorder badges in readme

### Miscellaneous Tasks

- Remove windows arm target
- Update claude settings permissions
- Update allowed permissions in claude settings
## [0.1.4] - 2026-02-17

### Other

- Setup cargo-dist for releases
- Refactor release hook configuration

### Miscellaneous Tasks

- Add cliff
- Configure cargo-release with git-cliff integration
- Release v0.1.3
- Fix git-cliff workdir in release hook
- Update cliff
- Release v0.1.4
## [papers-openalex-v0.1.2] - 2026-02-17

### Miscellaneous Tasks

- Release
## [papers-openalex-v0.1.1] - 2026-02-17

### Features

- Add openalex rest api
- Implement papers-mcp server and update openalex types
- Reconstruct abstract from inverted index in openalex
- Use slim summary structs for list endpoints
- Add papers-cli and extract shared papers crate
- Add support for topic hierarchy entities
- Add domain, field, and subfield entities
- Add shorthand filter aliases for work list
- Implement disk caching for openalex client
- Add shorthand filter aliases to all list endpoints
- Implement smart ID resolution for get endpoints

### Other

- Use workspace inheritance for common dependencies
- Update workspace dependency management

### Refactor

- Add crates directory
- Rename tools to entity_verb pattern
- Rename openalex crate to papers-openalex
- Use WorkListParams for work_list API
- Remove deprecated concept autocomplete entity
- Rename papers crate to papers-core

### Documentation

- Expand openalex entity descriptions
- Add readmes
- Add initial readme for papers monorepo
- Refine crate descriptions in readme
- Update README with usage examples and crate descriptions
- Add link to papers-cli examples in readme
- Update documentation for papers and papers-openalex
- Update papers crate description in openalex readme
- Update readme with filter aliases and usage

### Miscellaneous Tasks

- Update tool permissions in claude settings
- Add github actions workflow for rust ci
- Add package metadata to cargo workspace
- Format authors as a list in cargo.toml
- Release
