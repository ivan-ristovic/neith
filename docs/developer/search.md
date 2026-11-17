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
- `regex-syntax`: safe literal extraction for regex prefiltering.
- `dirs`: live man cache directory resolution.
- `serde`: JSON result serialization.

## Search Flow

`SearchEngine::search`:

1. Normalizes plain queries; preserves regex text.
2. Returns catalog results for an empty query.
3. Adds live man-page results for `all` and `man` filters in the `all` library scope.
4. Adds indexed man catalog matches for man query syntax.
5. Runs Tantivy search for plain queries or scans catalog entries for regex queries.
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

Exact mode uses Tantivy query parsing for plain queries. If the query contains
regex metacharacters, Neith builds a case-insensitive regex and checks catalog
entries directly. It reads note bodies when the result filter permits content
matches. For patterns whose every branch has a required ASCII literal prefix
of at least three bytes, the catalog trigram filter can skip ASCII entries
without opening their files. Patterns without such prefixes, entries with
non-ASCII content, and old catalog entries without filters still receive the
full scan. The candidate list is bounded to twice the requested result limit.

## Live Man Lookup

`man::lookup_live_man`:

1. Parses the first query token as a possible man page.
2. Supports `name(section)` and `section name`.
3. Calls `man -wa`.
4. Renders pages with `man -l` piped to `col -b`.
5. Writes rendered text under the man cache directory.
6. Returns synthetic `SearchResult` values with library alias `live-man`.

The search engine keeps a small session cache of resolved man pages, including
unsuccessful lookups. The TUI worker can cancel obsolete searches between
entries, but a man subprocess already running completes before cancellation.

## Invariants

- `SearchResult` must remain serializable for JSON output.
- `body` and `is_live_man` are skipped in JSON.
- Live man failures return no results, not fatal errors.
- Result deduplication uses filesystem path string.
