# Developer Reference: Libraries

## Code

Primary file: `src/library.rs`.

Related files:

- `src/config.rs`: creates `Library` values.
- `src/indexer.rs`: calls discovery and signature APIs.
- `src/search.rs`: reads source kind strings from index/catalog entries.
- `src/note.rs`: uses library aliases and paths for note creation.

## Public Types

| Type | Role |
| --- | --- |
| `Library` | Alias, root path, pinned flag, cache directory helper. |
| `SourceKind` | `Note`, `Devdocs`, or `Man`. |
| `EntryDoc` | Full discovered Markdown entry used for indexing. |
| `FileSignature` | Stable file metadata used for incremental indexing. |

## Public Functions

| Function | Role |
| --- | --- |
| `discover_markdown_entries` | Walk a library and return `EntryDoc` values. |
| `discover_signatures` | Return `FileSignature` values without exposing full bodies. |
| `read_entry` | Build one `EntryDoc` from a Markdown path. |
| `extract_title` | Return the first Markdown `# ` heading. |
| `title_from_path` | Derive a title from filename. |
| `content_hash` | Stable FNV-style body hash. |
| `infer_alias` | Derive a library alias from a path. |

## Dependencies

Internal consumers:

- `src/config.rs`: creates `Library` values.
- `src/indexer.rs`: consumes entries and file signatures.
- `src/search.rs`: consumes source kind strings through index/catalog data.
- `src/note.rs`: uses library aliases and paths for note creation.

External crates:

- `walkdir`: directory traversal without symlink following.
- `serde`: manifest/catalog serialization for exported types.
- `anyhow`: discovery and file-read error context.

## Discovery Flow

`discover_markdown_entries`:

1. Loads `.neithignore` from the library root.
2. Walks the library with `WalkDir`.
3. Skips built-in ignored directories.
4. Applies `.neithignore`.
5. Keeps only regular `.md` files.
6. Calls `read_entry`.
7. Sorts by relative path.

## `.neithignore`

Implementation type: private `LibraryIgnore`.

Supported rules:

- blank lines ignored
- `#` comments ignored
- exact relative paths
- directory prefixes ending in `/`

The matcher receives the library root and candidate path so it can evaluate
relative paths consistently.

## Source Classification

`classify_source` is private to `src/library.rs`. The result is stored in
`EntryDoc` and `FileSignature`.

Classification affects:

- Tantivy field `source_kind`
- manifest signatures
- catalog entries
- search filtering
- status kind

Changing classification rules may make existing manifests stale.

## Invariants

- Symlink following is disabled.
- Only `.md` files are entries.
- Excerpts and snippets must preserve UTF-8 boundaries.
- Cache directories are derived from `Library::cache_dir`.
