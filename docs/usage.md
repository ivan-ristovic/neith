# Usage

## Start The TUI

```sh
neith
```

Start with a query:

```sh
neith awk print 3rd column
```

Append libraries for one invocation:

```sh
neith --libs "$HOME/neith/neith-lib:$HOME/neith/other-lib" awk fields
```

Use a specific config:

```sh
neith --config ./config.toml
```

## Commands

| Command | Description |
| --- | --- |
| `neith [query...]` | Start the TUI. |
| `neith --config PATH` | Use a specific config file. |
| `neith --libs PATHS` | Append colon-separated libraries. |
| `neith --rebuild` | Rebuild indexes before TUI or JSON query startup. |
| `neith index` | Build or update indexes. |
| `neith index --rebuild` | Remove and rebuild indexes. |
| `neith status` | Print index/cache status. |
| `neith status --json` | Print status as JSON. |
| `neith healthcheck` | Check config, libraries, indexes, editor, clipboard, `man`, and `col`. |
| `neith healthcheck --json` | Print healthcheck as JSON. |
| `neith add <query...>` | Create or open a note and launch the editor. |
| `neith config init` | Write config from resolved runtime libraries. |
| `neith config init --force` | Overwrite an existing config. |
| `neith completions bash` | Print bash completions. |
| `neith completions zsh` | Print zsh completions. |
| `neith json query <query...>` | Print search results as JSON. |
| `neith json query <query...> --limit N` | Limit JSON results. |

## TUI Keys

| Key | Action |
| --- | --- |
| `Tab` | Switch results and preview focus. |
| `Ctrl-H` | Open or close help. |
| `Ctrl-Q` | Quit from any mode. |
| `Esc` | Cancel popup/copy/focus, or quit from results. |
| `Ctrl-A` | Add a new library entry from the current query. |
| `Ctrl-C` | Quick-copy the selected note payload. |
| `Ctrl-O` | Open the selected result in a new tmux pane. |
| `Ctrl-W` | Delete the previous query word. |
| `Ctrl-X` | Toggle exact/fuzzy query mode. |
| `Ctrl-F` | Filter over current results. |
| `Ctrl-T` | Cycle result type: `all`, `names`, `content`, `man`. |
| `Ctrl-L` | Cycle pinned libraries or open library picker. |
| Typing | Edit the query in results mode. |
| `Backspace` | Delete query text in results mode. |
| `Up/Down` | Move through results. |
| `j/k` | Move through preview lines or picker rows. |
| `PageUp/PageDown` | Scroll preview by a page. |
| `Shift-Up/Shift-Down` | Scroll preview by one line. |
| `Enter` in results | Open selected result. |
| `Enter/Space` in preview | Start copy selection; copy selection when already selecting. |
| `v` in preview copy mode | Move selection anchor to current preview line. |
| `1-9` in quick-copy chooser | Copy numbered code block. |

## JSON Query

```sh
neith json query awk print 3rd column --limit 10
```

JSON results include title, path, relative path, library alias, source kind,
line, snippet, score, and rank reason.

Related docs:

- [Search](search.md)
- [Quick-Copy](quick-copy.md)
- [Note Creation](note-creation.md)
- [tmux](tmux.md)
- [Developer TUI](developer/tui.md)
