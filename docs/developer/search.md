# Developer Reference: Search

## Code

Primary files:

- `src/search.rs`
- `src/query.rs`
- `src/man.rs`

Related files:

- `src/indexer.rs`: index handles, schema fields, catalog.
- `src/tui.rs`: constructs `SearchRequest` from UI state.
- `src/main.rs`: constructs JSON query requests.

## Public Types

| Type | File | Role |
| --- | --- | --- |
| `SearchResult` | `search.rs` | Result returned to TUI and JSON. |
| `SearchEngine` | `search.rs` | Search coordinator. |
| `SearchRequest` | `query.rs` | Query text, filter, mode, library scope, limit. |
| `SourceFilter` | `query.rs` | `all`, `names`, `content`, `man`. |
| `MatchMode` | `query.rs` | `fuzzy` or `exact`. |
| `LibraryScope` | `query.rs` | `all` or one alias. |
| `ManQuery` | `man.rs` | Parsed man-page query. |

## Dependencies

Internal:

- `src/indexer.rs`: `IndexManager`, `IndexHandle`, `CatalogEntry`, and
  Tantivy field handles.
- `src/query.rs`: normalized request model and query helpers.
- `src/man.rs`: live man-page parsing and rendering.
- `src/tui.rs` and `src/main.rs`: construct requests for TUI and JSON search.

External crates:

- `tantivy`: indexed search and result loading.
- `nucleo-matcher`: fuzzy catalog fallback.
- `regex`: exact-mode verification.
- `dirs`: live man cache directory resolution.
- `serde`: JSON result serialization.

## Search Flow

`SearchEngine::search`:

1. Normalizes the query.
2. Returns catalog results for an empty query.
3. Adds live man-page results for `all` and `man` filters.
4. Adds indexed man catalog matches for man query syntax.
5. Runs Tantivy search.
6. Runs fuzzy catalog fallback when fuzzy mode returns no results.
7. Deduplicates by path.
8. Sorts by score, then display line.
9. Truncates to the request limit.

## Query Normalization

`query::normalize_query` maps ordinal-like terms to `selected`:

- `nth`
- `1st`, `2nd`, `3rd`, `4th`, etc.
- awk-style `$3`

This is intentionally small and query-domain-specific.

## Exact Mode

Exact mode still uses Tantivy query parsing. If the query contains regex
metacharacters, Neith also builds a case-insensitive regex and verifies matches
against result text before accepting them.

## Live Man Lookup

`man::lookup_live_man`:

1. Parses the first query token as a possible man page.
2. Supports `name(section)` and `section name`.
3. Calls `man -wa`.
4. Renders pages with `man -l` piped to `col -b`.
5. Writes rendered text under the man cache directory.
6. Returns synthetic `SearchResult` values with library alias `live-man`.

## Invariants

- `SearchResult` must remain serializable for JSON output.
- `body` and `is_live_man` are skipped in JSON.
- Live man failures return no results, not fatal errors.
- Result deduplication uses filesystem path string.
