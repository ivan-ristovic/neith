# Neith

## Summary

Neith is a terminal search tool for Markdown knowledge libraries. It indexes
configured libraries with Tantivy, searches notes, generated DevDocs pages, and
man-page entries, and opens selected results directly in your editor.

Each library keeps its own cache under `.neith-cache/neith/`. The main config
file is TOML at `${XDG_CONFIG_HOME:-~/.config}/neith/config.toml`.

## Features

- Fast indexed search over Markdown libraries.
- Fuzzy and exact query modes.
- Result types for names, content, and man pages.
- Live man-page lookup with rendered man-page caching.
- TUI preview pane with syntax highlighting, line selection, and clipboard copy.
- Editor integration for opening results and creating new entries.
- `status`, `healthcheck`, and JSON query commands for automation.
- Bash and zsh completion generation.

## Dependencies

Build dependencies:

- Rust toolchain with Cargo.

Runtime dependencies:

- An editor command, from `editor.command`, `$EDITOR`, or `vi`.
- `wl-copy`, `xclip`, `xsel`, `tmux`, or `clipboard.command` for clipboard copy support.
- `man` and `col` for live man-page results.
- Optional: `bat` or `batcat` for syntax-highlighted previews.
- `tmux` for popup use and `Ctrl-O` pane opens.

The `install` and `uninstall` scripts use `sudo` for system paths under
`/usr/local/bin`, `/usr/share/bash-completion`, and
`/usr/local/share/zsh/site-functions`.

## Installation

Build from source:

```sh
cargo build --release
```

Install the release binary and shell completions:

```sh
./install
```

The installer symlinks the built binary to `/usr/local/bin/neith` and writes
bash and zsh completions.

Uninstall:

```sh
./uninstall
```

The uninstaller removes the installed completion files. It removes
`/usr/local/bin/neith` only when that path is a symlink to this checkout.

## Configuration

Create or edit:

```text
${XDG_CONFIG_HOME:-~/.config}/neith/config.toml
```

Example:

```toml
[[libraries]]
path = "~/neith/neith-lib"
alias = "neith-lib"
pinned = true

[[libraries]]
path = "~/neith/neith-devdocs/generated"
alias = "devdocs"
pinned = true

[editor]
command = "nvim"
return_behavior = "resume"

[clipboard]
command = "xclip -sel clip"

[ui]
preview_cursor_percent = 50
preview_syntax = "auto"
preview_bat_args = []

[ui.prompt]
separator = ":"
right_separator = ">"
```

Library paths are loaded from config first, then `NEITH_LIBS`, then `--libs`.
`~/` is expanded at the start of a path. Shell variables such as `$HOME` are not
expanded inside config strings. `preview_cursor_percent` controls where the
preview cursor sits in the visible preview pane while scrolling; `50` keeps it
near the middle. `preview_syntax` accepts `auto`, `plain`, or `bat`; `auto` uses
`bat` or `batcat` when found and falls back to plain preview. `preview_bat_args`
adds extra argv entries after Neith's safe `bat` defaults and before the file
path. Set `clipboard.command` to override clipboard copy; Neith writes copied
text to the command's stdin.

See `config-sample.toml` for all supported config fields, including prompt
colors.

Generate a config from currently resolved libraries:

```sh
NEITH_LIBS="$HOME/neith/neith-lib:$HOME/neith/neith-devdocs/generated" neith config init
```

## Usage

Start the TUI:

```sh
neith
```

Start with an initial query:

```sh
neith awk print 3rd column
```

Run with explicit libraries:

```sh
neith --libs "$HOME/neith/neith-lib:$HOME/neith/neith-devdocs/generated" awk fields
```

Build or refresh indexes:

```sh
neith index
neith index --rebuild
```

Check library and cache state:

```sh
neith status
neith healthcheck
```

Query from scripts:

```sh
neith json query awk print 3rd column --limit 10
```

## Commands

| Command | Description |
| --- | --- |
| `neith [query...]` | Start the TUI, optionally with an initial query. |
| `neith --config PATH` | Use an explicit TOML config path. |
| `neith --libs PATHS` | Append colon-separated library paths. |
| `neith --rebuild` | Rebuild indexes before TUI or JSON query startup. |
| `neith index` | Incrementally build or update all configured indexes. |
| `neith index --rebuild` | Remove and rebuild all configured library indexes. |
| `neith status` | Print per-library index/cache status. |
| `neith status --json` | Print status rows as JSON. |
| `neith healthcheck` | Check config, libraries, indexes, editor, clipboard, and man tools. |
| `neith healthcheck --json` | Print health checks as JSON. |
| `neith add <query...>` | Create a note from the library template and open it in the editor. |
| `neith config init` | Write a TOML config from resolved runtime libraries. |
| `neith completions bash` | Print bash completions to stdout. |
| `neith completions zsh` | Print zsh completions to stdout. |
| `neith json query <query...>` | Search all libraries and print JSON results. |

## TUI Keys

| Key | Action |
| --- | --- |
| `Tab` | Switch results and preview focus. |
| `Ctrl-A` | Add a new library entry from the current query. |
| `Ctrl-X` | Toggle exact/fuzzy query mode. |
| `Ctrl-F` | Filter over the current result list. |
| `Ctrl-T` | Cycle result types: `all`, `names`, `content`, `man`. |
| `Ctrl-L` | Cycle pinned libraries or open the library selector. |
| `Ctrl-H` | Open or close help. |
| `Ctrl-C` | Quick-copy the selected note payload. |
| `Ctrl-O` | Open the selected result in a new tmux pane and exit Neith. |
| `Enter` in results | Open the selected result in the editor. |
| `Enter` in add prompt | Create/open the edited note path. |
| `Enter` or `Space` in preview | Start or finish copy selection. |
| `v` in copy mode | Move the selection anchor to the current line. |
| `Up/Down` | Move result selection or preview cursor. |
| `j/k` | Move through the preview. |
| `PageUp/PageDown` | Scroll preview text by a page without changing focus or selection. |
| `Shift+Up/Shift+Down` | Scroll preview text by one line without changing focus or selection. |
| Mouse wheel over preview | Scroll preview text without changing focus or selection. |
| `Ctrl-Q` | Quit from any mode. |
| `Esc` | Cancel popup/copy/focus, or quit from results. |

Quick-copy uses marked Markdown regions when present:

````md
<!-- copy_begin -->
```bash
fd -e md
```
<!-- copy_end -->
````

If a note has no copy region, `Ctrl-C` copies the only fenced code block. If
there are multiple code blocks, Neith opens a chooser with `1-9` shortcuts,
arrow navigation, and `Enter` to copy.

## Shell Completions

The install script writes completions automatically. To install manually,
redirect generated output to a directory loaded by your shell:

```sh
neith completions bash > ~/.local/share/bash-completion/completions/neith
neith completions zsh > ~/.local/share/zsh/site-functions/_neith
```

## tmux Popup

Bind Neith to a tmux popup:

```sh
bind-key -r N run-shell "/path/to/neith/tmux_popup"
```

Popup sizing can be adjusted with `NEITH_POPUP_WIDTH` and
`NEITH_POPUP_HEIGHT`.

From the popup, `Ctrl-O` opens the selected result in a new tmux split, closes
Neith, and leaves a shell in that pane after the editor exits. Neith uses a
vertical split for wide panes and a horizontal split otherwise.

## Development

Run the test script:

```sh
./test
```

Developer documentation lives in `DOCS.md`.
