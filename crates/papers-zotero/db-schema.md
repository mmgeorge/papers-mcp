# Zotero SQLite Database — Local Access Reference

Zotero stores all library data in a single SQLite file:

| Platform | Default path |
|----------|-------------|
| Windows  | `%APPDATA%\Zotero\Zotero\zotero.sqlite` (i.e. `C:\Users\<user>\Zotero\zotero.sqlite`) |
| macOS    | `~/Zotero/zotero.sqlite` |
| Linux    | `~/Zotero/zotero.sqlite` |

## Opening While Zotero Is Running

Zotero holds an exclusive WAL write lock while open.  Opening normally fails
with `database is locked`.  The `immutable=1` URI flag bypasses this safely
for **read-only** access:

```rust
// rusqlite
let conn = rusqlite::Connection::open_with_flags_and_vfs(
    path,
    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    "unix-none",  // or platform default
)?;
// Or via URI string:
// "file:///path/to/zotero.sqlite?mode=ro&immutable=1"
```

```python
import sqlite3
conn = sqlite3.connect("file:///path/to/zotero.sqlite?mode=ro&immutable=1", uri=True)
```

`immutable=1` skips shared-memory WAL files and reads the DB snapshot as-is.
Data may be up to one commit behind if Zotero is actively writing, but this is
fine for all read-use-cases here.

---

## Schema Version (as of Zotero 7.0.32)

```
system        32
userdata      123
triggers      18
compatibility  7
delete        74
globalSchema  40
```

The `version` table (`schema TEXT PK, version INT`) tracks each schema
subsystem independently. `userdata` version is the one most likely to change
across Zotero releases.

---

## Tables by Category

### Identity & Library

#### `libraries` (1 row)
The single user library (group libraries would add rows).

| Column | Type | Notes |
|--------|------|-------|
| `libraryID` | INTEGER PK | Always `1` for the personal library |
| `type` | TEXT | `"user"` |
| `editable` | INT | |
| `filesEditable` | INT | |
| `version` | INT | Current library version (matches `fulltext_1` in `version` table) |
| `storageVersion` | INT | |
| `lastSync` | INT | Unix timestamp of last sync |
| `archived` | INT | |

#### `users` (1 row)

| Column | Type | Notes |
|--------|------|-------|
| `userID` | INTEGER PK | e.g. `16916553` |
| `name` | TEXT | e.g. `"mattmg"` |

#### `settings` (6 rows — local client settings, NOT synced)
Miscellaneous local-only key/value store. Some values are BLOBs (compressed JSON).

| Column | Type |
|--------|------|
| `setting` | TEXT PK | Namespace e.g. `"account"`, `"client"`, `"globalSchema"` |
| `key` | TEXT PK | |
| `value` | (any) | String, integer, or BLOB |

Useful non-blob rows:

| setting | key | value |
|---------|-----|-------|
| `account` | `username` | Zotero username |
| `account` | `userID` | Numeric user ID |
| `account` | `localUserKey` | Local user key |
| `client` | `lastVersion` | Zotero version string e.g. `"7.0.32"` |
| `client` | `lastCompatibleVersion` | e.g. `"6.0.36"` |

---

### Bibliographic Items

#### `items` (1117 rows)
Top-level item registry. Every item (work, attachment, note, annotation) has a row here.

| Column | Type | Notes |
|--------|------|-------|
| `itemID` | INTEGER PK | Internal integer ID |
| `itemTypeID` | INT | FK → `itemTypes` |
| `dateAdded` | TIMESTAMP | |
| `dateModified` | TIMESTAMP | |
| `clientDateModified` | TIMESTAMP | |
| `libraryID` | INT | FK → `libraries` |
| `key` | TEXT | 8-char Zotero key (e.g. `LF4MJWZK`) |
| `version` | INT | Library version at last modification |
| `synced` | INT | `1` = synced to server |

#### `itemTypes` / `itemTypesCombined` (40 types)
Maps `itemTypeID` → type name.  Notable types:

| ID | typeName |
|----|----------|
| 1  | annotation |
| 3  | attachment |
| 7  | book |
| 8  | bookSection |
| 11 | conferencePaper |
| 14 | document |
| 19 | journalArticle |
| 26 | preprint |
| 27 | report |
| 29 | thesis |
| 33 | webpage |

#### `itemData` + `itemDataValues` (5303 / 3284 rows)
Stores all bibliographic field values as a normalized EAV table.

```sql
-- Get all fields for an item by key
SELECT f.fieldName, idv.value
FROM items i
JOIN itemData id ON id.itemID = i.itemID
JOIN fieldsCombined f ON f.fieldID = id.fieldID
JOIN itemDataValues idv ON idv.valueID = id.valueID
WHERE i.key = 'LF4MJWZK'
```

#### `fields` / `fieldsCombined` (123 fields)
Maps `fieldID` → field name (e.g. `title`, `DOI`, `url`, `date`, `abstractNote`).

#### `creators` (871 rows) + `itemCreators` (1116 rows) + `creatorTypes` (37 types)

```sql
-- Get creators for an item
SELECT c.firstName, c.lastName, ct.creatorType
FROM items i
JOIN itemCreators ic ON ic.itemID = i.itemID
JOIN creators c ON c.creatorID = ic.creatorID
JOIN creatorTypes ct ON ct.creatorTypeID = ic.creatorTypeID
WHERE i.key = 'LF4MJWZK'
ORDER BY ic.orderIndex
```

---

### Attachments

#### `itemAttachments` (464 rows)

| Column | Type | Notes |
|--------|------|-------|
| `itemID` | INTEGER PK | FK → `items` |
| `parentItemID` | INT | FK → `items` (the parent work) |
| `linkMode` | INT | 0=imported_file, 1=imported_url, 2=linked_file, 3=linked_url |
| `contentType` | TEXT | MIME type e.g. `"application/pdf"` |
| `charsetID` | INT | FK → `charsets` |
| `path` | TEXT | Relative storage path e.g. `"storage:filename.pdf"` |
| `syncState` | INT | |
| `storageModTime` | INT | mtime in ms |
| `storageHash` | TEXT | MD5 hex |
| `lastProcessedModificationTime` | INT | |

Storage files live at `{zotero_data}/storage/{key}/{filename}`.

```sql
-- Get PDF path for a work's primary attachment
SELECT i.key, ia.contentType, ia.path
FROM items parent
JOIN itemAttachments ia ON ia.parentItemID = parent.itemID
JOIN items i ON i.itemID = ia.itemID
WHERE parent.key = 'LF4MJWZK'
  AND ia.contentType = 'application/pdf'
```

---

### Notes & Annotations

#### `itemNotes` (41 rows)

| Column | Type | Notes |
|--------|------|-------|
| `itemID` | INTEGER PK | FK → `items` |
| `parentItemID` | INT | FK → `items` (parent work or attachment) |
| `note` | TEXT | HTML content |
| `title` | TEXT | Auto-generated plain-text title |

#### `itemAnnotations` (205 rows)
PDF highlights, underlines, notes, images, ink.

| Column | Type | Notes |
|--------|------|-------|
| `itemID` | INTEGER PK | FK → `items` |
| `parentItemID` | INT | FK → `items` (attachment) |
| `type` | INTEGER | 1=highlight, 2=note, 3=image, 4=ink, 5=underline, 6=text |
| `authorName` | TEXT | |
| `text` | TEXT | Selected/highlighted text |
| `comment` | TEXT | User comment |
| `color` | TEXT | Hex color e.g. `"#2ea8e5"` |
| `pageLabel` | TEXT | Display page label e.g. `"594"` |
| `sortIndex` | TEXT | `"pageIndex\|charOffset\|lineIndex"` |
| `position` | TEXT | JSON with `pageIndex` and `rects` array |
| `isExternal` | INT | 1 = from external annotator |

```sql
-- All annotations on an attachment
SELECT ia.type, ia.text, ia.comment, ia.color, ia.pageLabel, ia.position
FROM items att
JOIN itemAnnotations ia ON ia.parentItemID = att.itemID
WHERE att.key = 'BGM2NJZH'
ORDER BY ia.sortIndex
```

---

### Collections

#### `collections` (58 rows)

| Column | Type | Notes |
|--------|------|-------|
| `collectionID` | INTEGER PK | |
| `collectionName` | TEXT | |
| `parentCollectionID` | INT | NULL = top-level |
| `clientDateModified` | TIMESTAMP | |
| `libraryID` | INT | |
| `key` | TEXT | 8-char key |
| `version` | INT | |
| `synced` | INT | |

#### `collectionItems` (394 rows)

| Column | Type |
|--------|------|
| `collectionID` | INT PK |
| `itemID` | INT PK |
| `orderIndex` | INT |

```sql
-- Items in a collection by name
SELECT i.key, idv.value as title
FROM collections c
JOIN collectionItems ci ON ci.collectionID = c.collectionID
JOIN items i ON i.itemID = ci.itemID
LEFT JOIN itemData id ON id.itemID = i.itemID
LEFT JOIN fieldsCombined f ON f.fieldID = id.fieldID AND f.fieldName = 'title'
LEFT JOIN itemDataValues idv ON idv.valueID = id.valueID
WHERE c.collectionName = 'GPU'
```

---

### Tags

#### `tags` (43 rows)

| Column | Type |
|--------|------|
| `tagID` | INTEGER PK |
| `name` | TEXT |

#### `itemTags` (120 rows)

| Column | Type | Notes |
|--------|------|-------|
| `itemID` | INT PK | |
| `tagID` | INT PK | |
| `type` | INT | 0 = manual, 1 = automatic |

```sql
-- All items with a specific tag
SELECT i.key
FROM tags t
JOIN itemTags it ON it.tagID = t.tagID
JOIN items i ON i.itemID = it.itemID
WHERE t.name = 'Starred'
```

---

### Full-Text Index

#### `fulltextItems` (402 rows)
Tracks indexing state for attachment items.

| Column | Type | Notes |
|--------|------|-------|
| `itemID` | INTEGER PK | FK → `items` (attachment) |
| `indexedPages` | INT | NULL for non-PDFs |
| `totalPages` | INT | NULL for non-PDFs |
| `indexedChars` | INT | NULL for PDFs |
| `totalChars` | INT | NULL for PDFs |
| `version` | INT | Library version when indexed |
| `synced` | INT | |

The actual text is in `.zotero-ft-cache` alongside the PDF file in storage.

#### `fulltextWords` (103019 rows) + `fulltextItemWords` (842280 rows)
Inverted index for full-text search. `fulltextWords` maps `wordID → word`,
`fulltextItemWords` maps `(wordID, itemID)` pairs.

```sql
-- Items containing a word
SELECT DISTINCT i.key
FROM fulltextWords fw
JOIN fulltextItemWords fiw ON fiw.wordID = fw.wordID
JOIN items i ON i.itemID = fiw.itemID
WHERE fw.word = 'raytracing'
```

---

### Synced Settings

#### `syncedSettings` (135 rows)
Settings that sync to the Zotero server. This is the authoritative local copy.

| Column | Type | Notes |
|--------|------|-------|
| `setting` | TEXT PK | Key name e.g. `"tagColors"`, `"lastPageIndex_u_LF4MJWZK"` |
| `libraryID` | INT PK | |
| `value` | (any) | JSON string or scalar |
| `version` | INT | Library version when last modified |
| `synced` | INT | |

Notable keys:

| setting | value format |
|---------|-------------|
| `tagColors` | JSON array: `[{"name":"Starred","color":"#FF8C19"}, ...]` |
| `lastPageIndex_u_{key}` | String page number for PDF reader state |

```sql
-- Tag colors
SELECT value FROM syncedSettings WHERE setting = 'tagColors' AND libraryID = 1

-- Last-read page for an attachment
SELECT value FROM syncedSettings
WHERE setting = 'lastPageIndex_u_BGM2NJZH' AND libraryID = 1
```

---

### Sync & Deletion Tracking

#### `syncDeleteLog` (138 rows)
Records of synced objects deleted from the server side.

| Column | Type | Notes |
|--------|------|-------|
| `syncObjectTypeID` | INT | FK → `syncObjectTypes` |
| `libraryID` | INT | |
| `key` | TEXT | Object key |
| `dateDeleted` | TEXT | ISO datetime |

Currently only contains `syncObjectTypeID=7` (setting) rows — deleted
`lastPageIndex_u_*` settings from the server.

#### `syncObjectTypes` (7 rows)

| ID | name |
|----|------|
| 1  | collection |
| 2  | creator |
| 3  | item |
| 4  | search |
| 5  | tag |
| 6  | relation |
| 7  | setting |

#### `deletedItems` (1 row)
Items deleted locally but not yet synced.

| Column | Type |
|--------|------|
| `itemID` | INTEGER PK |
| `dateDeleted` | TIMESTAMP |

Also: `deletedCollections`, `deletedSearches` — same structure for their types.

#### `storageDeleteLog` (0 rows)
Tracks file storage deletions for sync.

| Column | Type |
|--------|------|
| `libraryID` | INT PK |
| `key` | TEXT PK | Attachment key |
| `dateDeleted` | TEXT |

#### `syncCache` (1149 rows)
Raw JSON snapshots of synced objects as last seen from the server.
Can be used to reconstruct API-format responses without a network call.

| Column | Type | Notes |
|--------|------|-------|
| `libraryID` | INT PK | |
| `key` | TEXT PK | |
| `syncObjectTypeID` | INT PK | |
| `version` | INT PK | |
| `data` | TEXT | Full JSON as returned by api.zotero.org |

```sql
-- Get cached API JSON for a specific item
SELECT data FROM syncCache
WHERE key = 'LF4MJWZK'
  AND syncObjectTypeID = 3   -- item
  AND libraryID = 1
ORDER BY version DESC LIMIT 1
```

This is potentially a high-value source: the cached JSON matches the remote
API shape exactly (includes `key`, `version`, `library`, `links`, `meta`,
`data` fields), and is present for all synced items.

---

### Saved Searches

#### `savedSearches` (1 row)

| Column | Type |
|--------|------|
| `savedSearchID` | INTEGER PK |
| `savedSearchName` | TEXT |
| `key` | TEXT |
| `version` | INT |
| `synced` | INT |

#### `savedSearchConditions` (1 row)

| Column | Type |
|--------|------|
| `savedSearchID` | INT PK |
| `searchConditionID` | INT PK |
| `condition` | TEXT | e.g. `"tag"` |
| `operator` | TEXT | e.g. `"is"` |
| `value` | TEXT | e.g. `"Starred"` |

---

### Lookup / Reference Tables (read-only schema data)

These tables are populated by Zotero's bundled schema and don't change with
user data.  Useful for translating IDs in queries.

| Table | Contents |
|-------|----------|
| `baseFieldMappings` | Maps item-type-specific fields to base fields |
| `charsets` | `charsetID → charset` name (40 charsets) |
| `creatorTypes` | `creatorTypeID → creatorType` (37 types: author, editor, translator, …) |
| `fieldFormats` | 3 formats: free text, integer, URL |
| `fields` / `fieldsCombined` | 123 bibliographic field definitions |
| `fileTypes` / `fileTypeMimeTypes` | 8 file types → MIME type mappings |
| `itemTypes` / `itemTypesCombined` | 40 item types |
| `itemTypeFields` | Which fields apply to each item type |
| `itemTypeCreatorTypes` | Which creator roles apply to each item type |
| `relationPredicates` | 1 predicate: `owl:sameAs` |

---

### Unused / Empty in Typical Personal Libraries

| Table | Notes |
|-------|-------|
| `customItemTypes` / `customFields` | Custom item type definitions (0 rows) |
| `groups` / `groupItems` | Group library membership (0 rows — no groups joined) |
| `feeds` / `feedItems` | RSS feed subscriptions (0 rows) |
| `publicationsItems` | Zotero My Publications (0 rows) |
| `retractedItems` | Flagged retracted papers (0 rows) |
| `proxies` / `proxyHosts` | Network proxies (0 rows) |
| `dbDebug1` | Debug table (0 rows) |

---

## Useful Composite Queries

### Full item metadata (API-equivalent)
```sql
SELECT
    i.key,
    it.typeName AS itemType,
    i.dateAdded,
    i.dateModified,
    i.version,
    f.fieldName,
    idv.value
FROM items i
JOIN itemTypesCombined it ON it.itemTypeID = i.itemTypeID
LEFT JOIN itemData id ON id.itemID = i.itemID
LEFT JOIN fieldsCombined f ON f.fieldID = id.fieldID
LEFT JOIN itemDataValues idv ON idv.valueID = id.valueID
WHERE i.libraryID = 1
  AND it.typeName NOT IN ('attachment', 'note', 'annotation')
ORDER BY i.key, f.fieldName
```

### Items modified since library version N (sync delta)
```sql
SELECT i.key, i.version
FROM items i
WHERE i.libraryID = 1 AND i.version > ?
ORDER BY i.version
```

### Deleted objects since version N
```sql
-- Items deleted locally (pre-sync)
SELECT di.dateDeleted, i.key
FROM deletedItems di
LEFT JOIN items i ON i.itemID = di.itemID

-- Objects deleted from server (already synced)
SELECT sdl.dateDeleted, sdl.key, sot.name AS objectType
FROM syncDeleteLog sdl
JOIN syncObjectTypes sot ON sot.syncObjectTypeID = sdl.syncObjectTypeID
WHERE sdl.libraryID = 1
```

### All annotations for a work (via attachment)
```sql
SELECT ia.type, ia.text, ia.comment, ia.color, ia.pageLabel,
       ia.sortIndex, ia.position, att.key AS attachmentKey
FROM items work
JOIN itemAttachments att_row ON att_row.parentItemID = work.itemID
JOIN items att ON att.itemID = att_row.itemID
JOIN itemAnnotations ia ON ia.parentItemID = att_row.itemID
WHERE work.key = 'LF4MJWZK'
ORDER BY ia.sortIndex
```

### Tag colors (from syncedSettings)
```sql
SELECT json_each.value
FROM syncedSettings, json_each(syncedSettings.value)
WHERE syncedSettings.setting = 'tagColors'
  AND syncedSettings.libraryID = 1
```

### Full-text word search
```sql
SELECT DISTINCT i.key
FROM fulltextWords fw
JOIN fulltextItemWords fiw ON fiw.wordID = fw.wordID
JOIN items i ON i.itemID = fiw.itemID
WHERE fw.word LIKE 'raytracing'
```

### `syncCache` as API response cache
```sql
-- Get the latest cached server response for any item
SELECT sc.key, sc.version, sc.data
FROM syncCache sc
WHERE sc.libraryID = 1
  AND sc.syncObjectTypeID = 3   -- items
ORDER BY sc.version DESC
LIMIT 25
```
