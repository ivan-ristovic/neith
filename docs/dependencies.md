# Dependencies

Neith has build-time, runtime, and optional integration dependencies.

## Build Dependencies

- Rust toolchain with Cargo.

The project uses Rust 2024 edition.

## Runtime Dependencies

| Dependency | Used for | Required |
| --- | --- | --- |
| Editor command | Opening selected results and created notes. | Yes |
| `man` | Discovering live man-page files. | For live man-page results |
| `col` | Rendering live man pages as plain text. | For live man-page results |
| Clipboard backend | Preview copy and quick-copy. | For copy support |

The editor command comes from `editor.command`, then `$EDITOR`, then `vi`.

Clipboard backends are tried in this order:

1. `clipboard.command` from config.
2. `wl-copy`.
3. `xclip -sel clip`.
4. `xsel --clipboard --input`.
5. `tmux set-buffer`, when running inside tmux.

## Optional Dependencies

| Dependency | Used for |
| --- | --- |
| `bat` or `batcat` | Syntax-highlighted previews. |
| `tmux` | Popup usage and `Ctrl-O` pane opens. |

If `preview_syntax = "auto"`, Neith uses `bat` or `batcat` when available and
falls back to plain preview.

## Rust Crates

Main crate dependencies:

| Crate | Role |
| --- | --- |
| `anyhow` | Error propagation and context. |
| `clap`, `clap_complete` | CLI parsing and shell completions. |
| `crossterm` | Terminal input/output and mouse support. |
| `ratatui` | TUI layout and rendering. |
| `tantivy` | Full-text indexing and search. |
| `nucleo-matcher` | Fuzzy catalog fallback matching. |
| `serde`, `serde_json`, `toml` | Config, manifests, JSON output. |
| `walkdir` | Library discovery. |
| `regex` | Exact-mode regex verification. |
| `dirs` | Config and cache directory resolution. |
| `ansi-to-tui` | Converting `bat` ANSI output for preview rendering. |
| `tempfile` | Tests. |

Related developer docs:

- [Developer Dependencies](developer/dependencies.md)
- [Developer Installation](developer/installation.md)
