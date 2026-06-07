# Neith Developer Reference

This document describes the Rust `neith` binary and the data model behind
library discovery, indexing, search, diagnostics, and the TUI. It is intended
for contributors changing behavior in `src/`, command output, or cache formats.

## Runtime Overview

`neith` searches configured Markdown libraries. Each library owns its own cache
under the library directory:

```text
<library>/.neith-cache/neith/
  tantivy/
  manifest.json
  catalog.json
```

The normal TUI startup path is:

1. `src/cli.rs` parses command-line flags and subcommands.
2. `src/config.rs` loads TOML config, environment libraries, and CLI libraries.
3. `src/main.rs` calls `ensure_ready_indexes`.
4. `src/indexer.rs` builds missing indexes or rebuilds requested indexes.
5. `IndexManager::open` opens one Tantivy reader per configured library.
6. `SearchEngine` receives search requests from `src/tui.rs`.
7. The TUI starts a background incremental index refresh after opening.

Subcommands use the same config and index code unless they have special startup
requirements. `neith config init` runs before normal config loading. `neith
healthcheck` runs its own load path so it can report config failures as checks.

## Module Map

| Module | Responsibility |
| --- | --- |
| `src/main.rs` | Command dispatch, index readiness, TUI startup, background refresh. |
| `src/cli.rs` | `clap` definitions for global flags and subcommands. |
| `src/config.rs` | TOML config loading, default paths, library precedence, editor config. |
| `src/library.rs` | Library model, Markdown discovery, source classification, signatures. |
| `src/indexer.rs` | Tantivy schema, index build/update, manifests, catalogs, status data. |
| `src/search.rs` | Search pipeline, filters, exact/fuzzy handling, ranking, snippets. |
| `src/man.rs` | Live man-page lookup, rendering, and cache files. |
| `src/tui.rs` | Ratatui UI, prompt state, key handling, editor return behavior. |
| `src/diagnostics.rs` | `status`, `healthcheck`, table rendering, color handling. |
| `src/action.rs` | Clipboard backends and editor process launch. |
| `src/note.rs` | `neith add` note path inference and starter note creation. |
| `src/query.rs` | Search request types, filters, modes, query normalization. |

## Configuration

The default config path is:

```text
${XDG_CONFIG_HOME:-~/.config}/neith/config.toml
```

The default is resolved through `dirs::config_dir()`. If `--config PATH` is
provided, that path replaces the default. The loader reads TOML only. It does
not read `neithrc`.

Minimal config:

```toml
[[libraries]]
path = "/home/ivan/neith/neith-lib"
alias = "neith-lib"
pinned = true

[[libraries]]
path = "/home/ivan/neith/neith-devdocs/generated"
alias = "devdocs"
pinned = true

[editor]
command = "nvim"
return_behavior = "resume"
```

Config fields:

| Field | Type | Behavior |
| --- | --- | --- |
| `libraries[].path` | path | Library root. `~/` is expanded. |
| `libraries[].alias` | string, optional | Display and filter name. Inferred when omitted. |
| `libraries[].pinned` | bool, optional | Adds the library to the fast `Ctrl-L` scope cycle. |
| `editor.command` | string | Command used by `Enter` and `neith add`. Defaults to `$EDITOR` or `vi`. |
| `editor.return_behavior` | `exit` or `resume` | Controls what happens after an editor exits from the TUI. |

Library sources are appended in this order:

1. TOML `[[libraries]]`.
2. `NEITH_LIBS`, split on `:`.
3. `--libs`, split on `:`.

After appending, libraries are deduplicated by canonical path when possible.
Missing or non-directory paths are dropped. Loading fails if no library remains.

Alias inference is in `Library::new` and `infer_alias`:

| Path pattern | Inferred alias |
| --- | --- |
| Contains `neith-devdocs/generated` | `devdocs` |
| Ends with `neith-lib` | `neith-lib` |
| Ends with `ol-docs` | `ol-docs` |
| Other path | Last non-empty component that is not `docs`, or `library` |

Default pinned aliases are `neith-lib`, `devdocs`, and `ol-docs`.

`neith config init` writes TOML to the default or explicit config path. It uses
the currently resolved runtime libraries, so `NEITH_LIBS` and `--libs` can seed
the generated file. Existing config files are preserved unless `--force` is set.

## Library Discovery

Libraries are plain directory trees. Discovery uses `walkdir` with symlink
following disabled.

Only files with a `.md` extension become index entries. The walker skips these
directories:

```text
.git
.neith-cache
target
.cache
```

For each Markdown file, `read_entry` builds an `EntryDoc`:

| Field | Source |
| --- | --- |
| `library_alias` | Resolved library alias. |
| `library_path` | Library root path. |
| `path` | Absolute or configured filesystem path to the file. |
| `rel_path` | Path relative to the library root. |
| `title` | First Markdown `# ` heading, or filename stem with `-` replaced by spaces. |
| `body` | Full Markdown text. |
| `excerpt` | First 12 non-empty lines joined with spaces, capped at 900 bytes. |
| `source_kind` | `note`, `devdocs`, or `man`. |
| `size` | File size in bytes. |
| `modified_unix` | File modification time as Unix seconds, or `0` if unavailable. |

Excerpt and snippet truncation preserve UTF-8 character boundaries.

### Source Classification

`classify_source` assigns one `SourceKind` to every entry.

| Rule | Source kind |
| --- | --- |
| Body starts with `# man:` | `man` |
| Library path contains `neith-devdocs/generated` | `devdocs`, except `man/` entries |
| Body contains `Generated from DevDocs` | `devdocs`, except `man/` entries |
| Body contains `DevDocs path:` | `devdocs`, except `man/` entries |
| Relative path starts with `man/` inside a DevDocs-generated library | `man` |
| No rule matched | `note` |

This classification is persisted in the Tantivy index, manifest, and catalog.
Changing classification rules can make existing manifests stale because
`source_kind` is part of the file signature.

## Index Storage

Each configured library has an independent index. There is no global index file.

Cache layout:

```text
<library>/.neith-cache/neith/
  tantivy/
    meta.json
    ...
  manifest.json
  catalog.json
```

`tantivy/` stores the full-text index. `manifest.json` stores signatures used
for incremental updates. `catalog.json` stores lightweight result metadata used
for empty-query output, man-page matching, and fuzzy fallback.

Live man-page rendering uses a separate user cache:

```text
${XDG_CACHE_HOME:-~/.cache}/neith/man/
```

### Manifest

`IndexManifest` has a format version and a list of `FileSignature` values:

```json
{
  "version": 1,
  "files": [
    {
      "path": "/home/ivan/neith/neith-lib/awk/print-fields.md",
      "rel_path": "awk/print-fields.md",
      "title": "Print Selected Fields With awk",
      "excerpt": "# Print Selected Fields With awk ...",
      "source_kind": "note",
      "size": 1024,
      "modified_unix": 1760000000,
      "content_hash": 1234567890
    }
  ]
}
```

The signature equality check controls incremental indexing. A file is unchanged
only when every signature field matches. The content hash is a stable FNV-style
64-bit hash over the full body.

`INDEX_VERSION` is currently `1`. If an incompatible manifest change is made,
update the version and make the rebuild path explicit. A version mismatch loads
as an empty manifest, which causes existing files to be treated as changed.

### Catalog

`catalog.json` is derived from the manifest after indexing:

```json
[
  {
    "library_alias": "neith-lib",
    "path": "/home/ivan/neith/neith-lib/awk/print-fields.md",
    "rel_path": "awk/print-fields.md",
    "title": "Print Selected Fields With awk",
    "excerpt": "# Print Selected Fields With awk ...",
    "source_kind": "note"
  }
]
```

The catalog intentionally omits body text. It is used when a result does not
need Tantivy scoring or full text.

### Tantivy Schema

The schema is built in `build_schema`:

| Field | Type options | Use |
| --- | --- | --- |
| `path_exact` | `STRING | STORED` | Unique delete key for replacing or removing docs. |
| `path_text` | `TEXT | STORED` | Searchable path text. |
| `rel_path` | `TEXT | STORED` | Display path and searchable path. |
| `library` | `STRING | STORED` | Library alias. |
| `title` | `TEXT | STORED` | Main name search field. |
| `body` | `TEXT | STORED` | Full content search field and preview/snippet source. |
| `excerpt` | `TEXT | STORED` | Short content search field and catalog preview. |
| `source_kind` | `STRING | STORED` | Filter and ranking input. |
| `size` | `STORED` | File size metadata. |
| `modified_unix` | `STORED` | File mtime metadata. |

`open_index` creates the index if it does not exist. If opening an existing
index fails, it removes `tantivy/` and creates a fresh index directory.

JSON writes for `manifest.json` and `catalog.json` are atomic at the file level:
the code writes a process-specific temporary file next to the target, then
renames it over the target.

## Index Lifecycle

`ensure_indexes(libraries, rebuild, progress)` iterates libraries in configured
order and returns one `IndexStats` value per library:

```rust
pub struct IndexStats {
    pub indexed: usize,
    pub removed: usize,
    pub unchanged: usize,
}
```

The per-library flow is:

1. Emit `scanning`.
2. If `rebuild` is true, remove the whole cache directory.
3. Create the cache directory.
4. Open or create the Tantivy index.
5. Create an `IndexWriter` with a 100 MB memory budget.
6. Emit `reading entries`.
7. Discover current Markdown entries.
8. Load the previous manifest, or use an empty one.
9. Delete Tantivy docs whose paths are in the old manifest but absent now.
10. Emit `indexing changed files`.
11. Build a new manifest from current entries.
12. For unchanged signatures, increment `unchanged`.
13. For changed signatures, delete the old doc by `path_exact`, add the new doc,
    and increment `indexed`.
14. Commit Tantivy changes if any file was indexed, any file was removed, or the
    manifest file was missing.
15. Write the new manifest.
16. Write the new catalog.

`neith index` runs this incremental path and prints:

```text
alias       indexed  removed  unchanged
neith-lib   12       0        408
devdocs     2        0        9298
```

`neith index --rebuild` removes each library cache before rebuilding it. The
global `--rebuild` flag is also honored by `neith index`, TUI startup, and JSON
query readiness checks.

## Index Readiness

`IndexManager::has_usable_indexes` is a fast startup check. It returns true only
when every configured library has:

```text
<cache>/tantivy/meta.json
<cache>/manifest.json
```

It does not compare current files with the manifest. The TUI can open a usable
but stale index, then the background refresh updates it. `neith status` performs
the slower stale check.

`IndexManager::open` opens every library index and loads every catalog. It uses
Tantivy readers with `ReloadPolicy::OnCommitWithDelay`. `IndexManager::reload`
requests a reader reload for all handles.

## Status Command

`neith status` discovers current signatures and compares them with each
manifest. Text output is a colorized table when stdout is a TTY and `NO_COLOR`
is unset:

```text
alias       kind       files  indexed  stale  cache     index
neith-lib   mixed      420    420      0      18M       ok
devdocs     devdocs    9300   9298     2      310M      stale
ol-docs     note       84     0        84     missing   missing
```

Columns:

| Column | Meaning |
| --- | --- |
| `alias` | Library alias. |
| `kind` | `note`, `devdocs`, `man`, `mixed`, or `empty`, based on current files. |
| `files` | Current discovered Markdown files. |
| `indexed` | Files recorded in `manifest.json`, or `0` without a manifest. |
| `stale` | Current files with changed signatures plus manifest files no longer present. |
| `cache` | Recursive cache directory size, or `missing`. |
| `index` | `ok`, `stale`, or `missing`. |

The `index` column is `missing` if any of these files are absent:

```text
<cache>/tantivy/meta.json
<cache>/manifest.json
<cache>/catalog.json
```

It is `stale` when all required files exist but `stale > 0`. It is `ok` when all
required files exist and `stale == 0`.

`neith status --json` serializes the same row model:

```json
[
  {
    "alias": "neith-lib",
    "kind": "mixed",
    "files": 420,
    "indexed": 420,
    "stale": 0,
    "cache": "18M",
    "index": "ok"
  }
]
```

## Healthcheck Command

`neith healthcheck` reports environment and runtime checks. Text output uses the
same table renderer and color policy as `status`.

Checks:

| Check | Level |
| --- | --- |
| Config load succeeds | `ok`; config parse/load error is `fail`. |
| Runtime has configured libraries | `ok`; missing libraries usually fail config load first. |
| Each library path can be read | `ok` or `fail`. |
| Duplicate aliases | `warn`. |
| Each cache/index status | `ok` for current indexes, `warn` for stale or missing indexes. |
| Editor executable exists | `ok` or `fail`. |
| Clipboard backend exists | `ok` or `warn`. |
| `man` and `col` exist | `ok` or `warn`. |

Exit codes:

| Code | Meaning |
| --- | --- |
| `0` | No warnings or failures. |
| `2` | At least one warning and no failures. |
| `1` | At least one failure. |

`neith healthcheck --json` serializes:

```json
{
  "checks": [
    {
      "level": "ok",
      "name": "config",
      "detail": "/home/ivan/.config/neith/config.toml"
    }
  ]
}
```

## Search Pipeline

The TUI and JSON query command build a `SearchRequest`:

```rust
pub struct SearchRequest {
    pub query: String,
    pub filter: SourceFilter,
    pub mode: MatchMode,
    pub library: LibraryScope,
    pub limit: usize,
}
```

The pipeline in `SearchEngine::search` is:

1. Normalize the query.
2. If the normalized query is empty, return catalog entries for matching
   libraries and filters.
3. For `all` and `man` filters, run live man lookup.
4. For `all`, `names`, and `man` filters, run catalog man-query matching.
5. Run Tantivy search.
6. If mode is fuzzy and no results exist yet, run catalog fuzzy fallback.
7. Deduplicate by full path, keeping the highest score for each path.
8. Sort by descending score, then `display_line`.
9. Truncate to the requested limit.

`SearchResult` serializes to JSON without `body` and `is_live_man`:

```json
{
  "title": "Print Selected Fields With awk",
  "path": "/home/ivan/neith/neith-lib/awk/print-fields.md",
  "rel_path": "awk/print-fields.md",
  "library_alias": "neith-lib",
  "source_kind": "note",
  "line": 12,
  "snippet": "awk '{ print $3 }' file",
  "score": 1234.5,
  "rank_reason": "index"
}
```

### Query Normalization

`normalize_query` rewrites common positional field terms to `selected`:

| Input token | Normalized token |
| --- | --- |
| `nth` | `selected` |
| `1st`, `2nd`, `3rd`, `4th`, etc. | `selected` |
| `$1`, `$2`, `$3`, etc. | `selected` |

This makes queries such as `awk print 3rd column` match notes that use
`selected fields`.

### Filters

`SourceFilter` changes both fields searched and source kinds allowed.

| Filter | Prompt label | Tantivy fields | Source behavior |
| --- | --- | --- | --- |
| `All` | omitted | `title`, `path_text`, `rel_path`, `body`, `excerpt` | Allows all sources. |
| `Names` | `names` | `title`, `path_text`, `rel_path` | Allows notes, DevDocs, and man entries. |
| `Content` | `content` | `body`, `excerpt` | Excludes man entries. |
| `Man` | `man` | `title`, `path_text`, `rel_path` | Allows only man entries. |

### Match Modes

`MatchMode::Fuzzy` builds a small set of Tantivy query variants. Variants include
man-query rewrites and aliases for `column`, `columns`, `field`, and `fields`.
If Tantivy and live man produce no results, fuzzy mode falls back to catalog
matching with `nucleo_matcher`.

`MatchMode::Exact` sends the normalized query directly to Tantivy unless the
query contains regex metacharacters. Regex exact mode extracts the longest
literal seed, uses that seed for Tantivy candidate retrieval, then verifies the
regex against title, relative path, full body, and snippet. Invalid regex
queries return no results.

### Snippets

Indexed results use `best_snippet` over the stored body. The first line
containing any query term is preferred. If no line matches, the first non-empty
line is used. Snippets are capped at 220 bytes on UTF-8 boundaries.

Regex exact mode can override the result line and snippet with the matching line.
Regex matches across body lines report the line containing the match start.

### Ranking

Tantivy BM25 or catalog fuzzy scores are adjusted by source, topic, filter, and
man-page boosts.

Important boost behavior:

| Condition | Effect |
| --- | --- |
| Live man result | Very large boost so live man pages rank above generated DevDocs man pages. |
| All query term groups match title or path | Source-specific boost: notes above DevDocs, man entries boosted separately. |
| First query topic matches top path segment or title | Additional source-specific boost. |
| `names` filter with all-name hit | Additional names boost. |
| `man` filter with man source | Additional man-filter boost. |
| Sectioned man query matches exact page | Large man-page boost. |
| Man query matches page prefix | Smaller man-page boost. |

The concrete values live in `apply_boosts` and `man_page_boost`. Tests cover the
expected ordering for selected-field queries, sectioned man queries, and live
man priority.

## Live Man Lookup

Live man lookup is intentionally outside the library index. It runs for `all`
and `man` filters before indexed search.

Accepted query shapes:

```text
printf(3) format
3 printf format
printf format
```

`split_man_query` parses those forms into:

```rust
pub struct ManQuery {
    pub title: String,
    pub section: Option<String>,
    pub remainder: String,
}
```

Lookup flow:

1. Resolve source files with `man -wa -- [section] title`.
2. Render each source file with `man -l -- <source> | col -b`.
3. Cache rendered text under `${XDG_CACHE_HOME:-~/.cache}/neith/man/`.
4. Return a `SearchResult` with:
   - `library_alias = "live-man"`
   - `source_kind = "man"`
   - `rank_reason = "live-man"`
   - `is_live_man = true`

Man lookup failures are non-fatal. If `man`, `col`, or a page is unavailable,
the lookup returns no live results and indexed search still runs.

## TUI Behavior

The TUI has four focus states:

| State | Meaning |
| --- | --- |
| `Results` | Query editing and result selection. |
| `Preview` | Preview navigation and copy selection. |
| `LibrarySelector` | Library picker opened from `Ctrl-L`. |
| `Help` | Help popup. |

Prompt format:

```text
<mode>:<library-scope>[:filter]> <query>
```

Examples:

```text
F:all> awk print selected column
E:all:names> awk.*print
F:devdocs:man> printf(3) format
```

Mode prefixes:

| Prefix | Mode |
| --- | --- |
| `F` | Fuzzy |
| `E` | Exact |

The filter segment is omitted for `all`. The library segment is `all` or a
library alias.

Key behavior:

| Key | Behavior |
| --- | --- |
| `Tab` | Switch results and preview focus; close popups. |
| `Ctrl-K` | Toggle fuzzy/exact query mode. |
| `Ctrl-R` | Toggle fuzzy refine over the current result list. |
| `Ctrl-T` | Cycle result filters: `all`, `names`, `content`, `man`. |
| `Ctrl-L` | Cycle pinned libraries, then open the library picker. |
| `Ctrl-H` | Open or close help. |
| `Ctrl-Q` | Quit from any mode. |
| `Esc` | Cancel popup/copy/focus, or quit from results. |
| `Enter` in results | Open the selected result in the configured editor. |
| `Enter` in library picker | Select the highlighted library scope. |
| `Enter` or `Space` in preview | Start copy mode, or copy the selected lines. |
| `v` in preview copy mode | Move the selection anchor to the current line. |
| `Up/Down` | Move result selection, preview cursor, or picker selection. |
| `j/k` | Move the preview cursor or picker selection. In results focus, `j` and `k` insert query text. |
| `PageUp/PageDown` | Move the preview cursor by a page. |
| Typing in results | Insert query text and refresh results. |
| `Backspace` in results | Delete the previous query character and refresh results. |

`Ctrl-R` result refine mode captures the currently displayed result list and the
current query. It clears the prompt for a refine query, then uses
`nucleo_matcher` to fuzzy-match that refine query against each captured result's
display line, title, and snippet. An empty refine query shows all captured
results. Pressing `Ctrl-R` again restores the captured query and result list.
Changing query mode, source filter, or library scope while refine mode is active
refreshes the captured base results and reapplies the refine query.

### Editor Flow

When `Enter` opens a result, the TUI stores an editor request with path and line,
restores the terminal, and runs:

```text
<editor command> +<line> <path>
```

The editor command is split on whitespace. It is executed directly, without a
shell.

`editor.return_behavior = "exit"` quits the TUI after the editor exits.

`editor.return_behavior = "resume"` initializes the terminal again after the
editor exits, refreshes indexes incrementally, reopens `IndexManager`, rebuilds
`SearchEngine`, and refreshes the current result list.

## Commands

| Command | Behavior |
| --- | --- |
| `neith` | Start the TUI with an empty query. |
| `neith <query...>` | Start the TUI with an initial query. |
| `neith --libs PATHS` | Append colon-separated library paths after config and `NEITH_LIBS`. |
| `neith --config PATH` | Use an explicit TOML config path. |
| `neith --rebuild` | Rebuild indexes before TUI or JSON search readiness. |
| `neith index` | Incrementally build or update all configured indexes. |
| `neith index --rebuild` | Remove and rebuild all configured library caches. |
| `neith status` | Print per-library index/cache status. |
| `neith status --json` | Print status rows as JSON. |
| `neith healthcheck` | Report config, library, index, editor, clipboard, man, and col checks. |
| `neith healthcheck --json` | Print health checks as JSON. |
| `neith add <query...>` | Create a starter note and open it in the editor. |
| `neith completions bash` | Print a bash completion script to stdout. |
| `neith completions zsh` | Print a zsh completion script to stdout. |
| `neith config init` | Write a TOML config from resolved runtime libraries. |
| `neith config init --force` | Overwrite an existing config. |
| `neith json query <query...>` | Search all libraries with fuzzy/all mode and print JSON. |
| `neith json query <query...> --limit N` | Set the JSON result limit. |

`neith json query` ensures indexes are usable before search. It uses:

```rust
SourceFilter::All
MatchMode::Fuzzy
LibraryScope::All
```

## Shell Completions

`neith completions bash` and `neith completions zsh` generate completion scripts
from the current `clap` command definition. They do not load runtime config and
do not require configured libraries.

The installer writes generated scripts to:

```text
/usr/share/bash-completion/completions/neith
/usr/local/share/zsh/site-functions/_neith
```

Manual installation can redirect the command output to a shell-specific
completion directory.

## Color and Table Rendering

`diagnostics::use_color_stdout` enables ANSI color only when stdout is a TTY and
`NO_COLOR` is unset. Tables compute widths from unstyled cell text, then apply
color to padded cells. This keeps colored and uncolored output aligned.

Status colors:

| Value | Style intent |
| --- | --- |
| `ok`, stale count `0` | Green. |
| `stale`, small stale count, `mixed` kind | Yellow. |
| `missing`, large stale count | Red. |
| Aliases | Cyan. |
| DevDocs kind | Blue. |
| Man kind | Magenta. |
| Cache sizes and unchanged counts | Dim. |

Index command colors:

| Column | Style |
| --- | --- |
| `alias` | Cyan. |
| Non-zero `indexed` | Green. |
| Non-zero `removed` | Yellow. |
| `unchanged` | Dim. |

## Note Creation

`neith add <query...>` creates a Markdown starter note before opening the editor.
The target library is the first library with alias `neith-lib`, or the first
configured library if `neith-lib` is absent.

Path inference:

1. Use the first query word as the category candidate.
2. If `<library>/<category>` exists, place the note in that directory.
3. Otherwise create/use a directory named from the sanitized first word.
4. Use the full query as the note filename slug.

Existing files are left unchanged. New notes include a generated title, the
original task text, a starter code block, and a references section.

## Clipboard

Preview copy uses `copy_text`:

1. If `xsel` exists, write to `xsel --clipboard --input`.
2. If inside tmux and `tmux` exists, run `tmux set-buffer -- <text>`.
3. Succeed if any backend succeeds.
4. Fail if no backend succeeds.

`healthcheck` warns when neither backend is available.

## Development Invariants

Keep these constraints in mind when changing behavior:

- Library caches are owned by the library directory. Avoid adding hidden global
  index state that would make per-library rebuilds incomplete.
- `manifest.json` is the incremental-index source of truth. Add fields to
  `FileSignature` only when changes to that field should force reindexing.
- `catalog.json` must stay derivable from the manifest. It should contain only
  lightweight metadata needed without full body text.
- Tantivy document replacement depends on `path_exact`. Changing path storage
  or normalization must preserve a unique delete key.
- Source classification affects filters, status `kind`, ranking, and stale
  detection.
- If the Tantivy schema changes incompatibly, make the migration behavior
  explicit. `open_index` currently recreates `tantivy/` only when open fails.
- Keep status and index text output stable unless the user-facing format is
  intentionally changing. Tests assert table spacing for some outputs.
- Preserve the `NO_COLOR` behavior and compute table widths before adding ANSI
  codes.
- `body` and `is_live_man` are skipped in JSON search output. Do not expose full
  body text through JSON accidentally.
- The editor command is not shell-expanded. If shell behavior is required, it
  must be represented explicitly in config, for example with a wrapper command.
- Symlinks are not followed during discovery. Changing that affects cache size,
  duplicate paths, and loop safety.

## Common Change Points

### Add a Config Field

1. Add the field to `AppConfig`, `LibraryConfig`, or `EditorConfig`.
2. Provide a `Default` or `serde(default)` path if existing configs should load.
3. Update `RuntimeConfig::load`.
4. Update `RuntimeConfig::init_config` so generated TOML includes the intended
   value.
5. Update this document and add a targeted test.

### Add a Source Kind

1. Add the variant to `SourceKind`.
2. Update `SourceKind::as_str` and `FromStr`.
3. Update `classify_source`.
4. Update status kind coloring if needed.
5. Update search `source_matches` and ranking boosts.
6. Decide whether existing manifests should become stale.

### Change Index Contents

1. Update `EntryDoc` if the data comes from library discovery.
2. Update `FileSignature` if changes to the data should trigger reindexing.
3. Update `CatalogEntry` if the data is needed without Tantivy.
4. Update the Tantivy schema and `entry_to_tantivy_doc`.
5. Update `doc_to_result` if search results need the field.
6. Consider `INDEX_VERSION` and old cache behavior.

### Change Ranking

1. Add or update focused tests in `src/search.rs`.
2. Keep source/filter behavior separate from score tuning.
3. Verify live man priority when changing man boosts.
4. Check both indexed and catalog fallback paths.

### Change TUI Keys

1. Update key handling in `App::handle_key` or focus-specific handlers.
2. Update the status header in `draw_results`.
3. Update `help_lines`.
4. Update README key documentation.
5. Update this document if the key is part of supported behavior.

## Testing

Use the project test script:

```sh
./test
```

The current test suite covers:

- Config defaults.
- Library title extraction, source classification, alias inference, and UTF-8
  truncation.
- Query normalization and regex seed extraction.
- Search snippets, source filters, ranking expectations, live man priority, and
  regex verification.
- TUI prompt formatting, cursor handling, focus behavior, and query editing.
- Diagnostics table rendering, status rows, and healthcheck exit codes.
- Note slug generation.

For index changes, prefer tests that create temporary libraries and call
`ensure_indexes`. For search ranking changes, construct `SearchResult` values
and call the ranking path where possible so expected ordering stays explicit.

## Troubleshooting Notes

| Symptom | Likely area |
| --- | --- |
| `neith` says no libraries configured | Config path, `NEITH_LIBS`, `--libs`, or dropped non-directories. |
| `status` shows `missing` | Cache directory, Tantivy `meta.json`, manifest, or catalog is absent. |
| `status` shows stale files | Signature changed, file added, or manifest still contains removed paths. |
| Live man results are absent | `man` or `col` missing, page unavailable, or query did not parse as a man query. |
| Editor opens wrong command | `editor.command` whitespace splitting; no shell parsing is performed. |
| Copy fails | No working `xsel` backend and no tmux backend. |
| JSON query is slow on first run | Indexes were missing or `--rebuild` forced an index pass before search. |
