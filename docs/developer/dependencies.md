# Developer Reference: Dependencies

## Cargo Dependencies

| Crate | Used in | Purpose |
| --- | --- | --- |
| `anyhow` | Most modules | Error context and propagation. |
| `clap` | `src/cli.rs`, `src/main.rs` | CLI model and parsing. |
| `clap_complete` | `src/main.rs` | Shell completion generation. |
| `crossterm` | `src/tui.rs` | Terminal events, raw mode, mouse capture. |
| `ratatui` | `src/tui.rs` | Layout and rendering. |
| `tantivy` | `src/indexer.rs`, `src/search.rs` | Full-text index and search. |
| `nucleo-matcher` | `src/search.rs` | Fuzzy catalog fallback. |
| `regex` | `src/query.rs`, `src/search.rs` | Regex metadata and exact-mode verification. |
| `serde`, `serde_json` | Config, indexer, diagnostics, search | JSON and data serialization. |
| `toml` | `src/config.rs` | Config parsing/writing. |
| `walkdir` | `src/library.rs` | Markdown discovery. |
| `dirs` | `src/config.rs`, `src/search.rs` | Config/cache directories and home expansion. |
| `ansi-to-tui` | `src/tui.rs` | Convert `bat` ANSI output to TUI spans. |
| `tempfile` | Tests | Temporary directories. |

## External Commands

| Command | Called from | Purpose |
| --- | --- | --- |
| editor command | `src/action.rs`, `src/main.rs`, `src/tui.rs` | Open files. |
| `wl-copy` | `src/action.rs` | Clipboard backend. |
| `xclip` | `src/action.rs` | Clipboard backend. |
| `xsel` | `src/action.rs` | Clipboard backend. |
| `tmux` | `src/action.rs`, `tmux_popup` | Clipboard fallback, popup, pane open. |
| `man` | `src/man.rs` | Resolve and render live man pages. |
| `col` | `src/man.rs` | Strip man-page control characters. |
| `bat`/`batcat` | `src/tui.rs` | Syntax-highlighted preview. |

## Dependency Boundaries

- `src/action.rs` owns process interaction for clipboard, editors, and tmux pane
  opens.
- `src/man.rs` owns `man` and `col` process interaction.
- `src/tui.rs` owns `bat` invocation for preview rendering.
- `src/diagnostics.rs` checks external command availability through
  `action::command_exists`.

## Invariants

- External command failure should become a status message, warning, or
  contextual error.
- Clipboard command strings are split on whitespace; shell syntax is not
  interpreted.
- Editor command strings are split on whitespace for normal opens and shell
  quoted for tmux pane opens.
