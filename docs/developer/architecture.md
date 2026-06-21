# Developer Reference: Architecture

## Code Map

| File | Responsibility |
| --- | --- |
| `src/main.rs` | Command dispatch, index readiness, TUI startup, background refresh. |
| `src/cli.rs` | `clap` CLI model and shell completion enums. |
| `src/config.rs` | TOML config loading, defaults, library precedence, config generation. |
| `src/library.rs` | Library model, Markdown discovery, `.neithignore`, source classification. |
| `src/indexer.rs` | Tantivy schema, manifests, catalogs, index status. |
| `src/search.rs` | Search orchestration, result ranking, snippet selection, catalog fallback. |
| `src/query.rs` | Query mode/filter/scope types and query normalization. |
| `src/man.rs` | Live man-page lookup, rendering, and cache files. |
| `src/tui.rs` | Ratatui app state, rendering, key handling, editor return flow. |
| `src/action.rs` | Clipboard backends and editor/tmux process launching. |
| `src/note.rs` | Note path inference, slugging, template rendering. |
| `src/quick_copy.rs` | Markdown quick-copy region extraction. |
| `src/diagnostics.rs` | `status`, `healthcheck`, and table rendering. |
| `src/lib.rs` | Library module exports for tests and binary use. |

## Subsystem Groups

| Group | Modules |
| --- | --- |
| Startup and CLI | `main`, `cli`, `config` |
| Library model and indexing | `library`, `indexer` |
| Search | `search`, `query`, `man` |
| User interaction | `tui`, `action`, `note`, `quick_copy` |
| Inspection | `diagnostics` |
| Install/tmux scripts | `install`, `uninstall`, `release`, `tmux_popup` |

## Dependency Direction

`main` composes subsystems. Library discovery feeds indexing. Index handles
feed search. The TUI depends on search and action modules, while action modules
own external process calls. Diagnostics reads config, library, index, and tool
state without mutating indexes.

## Runtime Flow

Normal TUI startup:

1. `src/cli.rs` parses arguments into `Cli`.
2. `src/main.rs` handles early commands: completions, config init, healthcheck.
3. `RuntimeConfig::load` resolves config and libraries.
4. `ensure_ready_indexes` builds indexes if missing or requested by `--rebuild`.
5. `IndexManager::open` opens Tantivy readers and catalogs.
6. `SearchEngine::new` creates the search engine.
7. `maybe_refresh_in_background` starts an incremental index refresh.
8. `tui::run` starts the Ratatui event loop.

Subcommands share the same config and index modules unless they have explicit
early handling in `main.rs`.

## Data Flow

```text
config -> libraries -> discovery -> manifest/catalog/index -> search engine -> TUI/results
```

Important structs:

- `config::RuntimeConfig`
- `library::Library`
- `library::EntryDoc`
- `library::FileSignature`
- `indexer::IndexManager`
- `search::SearchEngine`
- `search::SearchResult`
- `query::SearchRequest`
- `tui::App`

## Invariants

- Each library owns its own cache.
- `catalog.json` is derived from `manifest.json`.
- `source_kind` is part of file signatures; classification changes can make
  manifests stale.
- The TUI should not require a full reindex after startup. Background refresh is
  incremental.
- Clipboard and editor commands are invoked through `src/action.rs`.
