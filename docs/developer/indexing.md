# Developer Reference: Indexing

## Code

Primary file: `src/indexer.rs`.

Related files:

- `src/library.rs`: discovery, source kind, file signatures.
- `src/main.rs`: calls `ensure_indexes` and `IndexManager::open`.
- `src/diagnostics.rs`: reads `library_status`.
- `src/search.rs`: consumes `IndexManager`, `IndexHandle`, and catalog entries.

## Public Types

| Type | Role |
| --- | --- |
| `SearchFields` | Tantivy field handles. |
| `IndexHandle` | Open index, reader, schema fields, library, catalog. |
| `CatalogEntry` | Lightweight result metadata. |
| `IndexManifest` | Format version and file signatures. |
| `IndexStats` | Indexed, removed, unchanged counts. |
| `LibraryStatus` | Status details for diagnostics. |
| `IndexManager` | Collection of open index handles. |

## Public Functions

| Function | Role |
| --- | --- |
| `IndexManager::open` | Open readers and catalogs for configured libraries. |
| `IndexManager::reload` | Reload all readers. |
| `IndexManager::has_usable_indexes` | Check for Tantivy meta and manifest files. |
| `ensure_indexes` | Build or update all configured library indexes. |
| `index_dir` | Return `<cache>/tantivy`. |
| `manifest_path` | Return `<cache>/manifest.json`. |
| `catalog_path` | Return `<cache>/catalog.json`. |
| `library_status` | Compute files/index/cache state. |

## Dependencies

Internal:

- `src/library.rs`: provides discovery, signatures, source kind, and cache
  paths.
- `src/search.rs`: opens `IndexManager` handles for query execution.
- `src/diagnostics.rs`: reads `library_status` for status and healthcheck.
- `src/main.rs`: calls index readiness and rebuild paths.

External crates:

- `tantivy`: index schema, writers, readers, queries, and document storage.
- `serde_json`: manifest and catalog persistence.
- `anyhow`: index and filesystem error context.

## Schema

Tantivy fields:

- `path_exact`
- `path_text`
- `rel_path`
- `library`
- `title`
- `body`
- `excerpt`
- `source_kind`
- `size`
- `modified_unix`

`path_exact`, `library`, and `source_kind` are string fields for exact matching.
`path_text`, `rel_path`, `title`, `body`, and `excerpt` are text fields.

## Update Flow

`ensure_library_index`:

1. Creates the library cache directory.
2. Opens or creates the Tantivy index.
3. Discovers current entries.
4. Loads previous manifest.
5. Deletes documents for removed paths.
6. Compares each current entry signature.
7. Reindexes changed entries.
8. Commits when documents changed or no manifest exists.
9. Writes manifest.
10. Writes catalog.

## Manifest And Catalog

`INDEX_VERSION` is currently `1`.

The manifest stores `FileSignature` values. The catalog is derived from the
manifest and stores metadata needed without loading full body text.

If an incompatible manifest format change is made, update `INDEX_VERSION`.
Version mismatch loads as an empty manifest, causing existing files to be
treated as changed.

## Invariants

- `catalog.json` must be derivable from `manifest.json`.
- Index rebuild must remove the whole library cache.
- Incremental updates must delete stale path terms before adding changed docs.
- Status must work even when cache files are missing.
