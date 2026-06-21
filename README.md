# Neith

![Neith search UI](docs/images/example-main.png)

Neith is a terminal search tool for Markdown knowledge libraries. In Egyptian
mythology, Neith was associated with wisdom and fate.

A library is a directory of Markdown notes. Neith indexes configured libraries
with Tantivy, searches notes, generated DevDocs pages, and man-page entries,
and opens selected results directly in your editor.

## Table of Contents

- [How It Works](#how-it-works)
- [Usage Screenshots](#usage-screenshots)
- [Setup](#setup)
- [Knowledge Libraries](#knowledge-libraries)
- [tmux Popup](#tmux-popup)
- [Documentation](#documentation)
- [Development](#development)

Detailed docs:

- [Docs Index](docs/README.md)
- [Installation](docs/installation.md)
- [Dependencies](docs/dependencies.md)
- [Configuration](docs/configuration.md)
- [Libraries](docs/libraries.md)
- [Indexing](docs/indexing.md)
- [Search](docs/search.md)
- [Usage](docs/usage.md)
- [Quick-Copy](docs/quick-copy.md)
- [Note Creation](docs/note-creation.md)
- [Diagnostics](docs/diagnostics.md)
- [tmux](docs/tmux.md)
- [Developer Reference](docs/developer/README.md)

## How It Works

Run `neith` to open the TUI, or pass an initial query:

```sh
neith grep pattern
```

Neith searches indexed note titles, paths, and bodies while you type. It can
also include generated DevDocs pages and live man-page results. Select a result
and press `Enter` to open it in your editor.

Common keys:

| Key | Action |
| --- | --- |
| `Tab` | Switch results and preview focus. |
| `Ctrl-T` | Cycle result types: `all`, `names`, `content`, `man`. |
| `Ctrl-F` | Filter within the current result list. |
| `Ctrl-L` | Cycle pinned libraries or open the library selector. |
| `Ctrl-C` | Quick-copy the selected note payload. |
| `Ctrl-A` | Create a note from the current query. |
| `Ctrl-O` | Open the selected result in a new tmux pane. |
| `Esc` | Cancel the current mode or quit from results. |

See [Usage](docs/usage.md) for the full command and key reference.

## Usage Screenshots

Start with a query and select a result:

![Initial query](docs/images/example-init.png)

Switch focus to inspect the preview:

![Preview pane](docs/images/example-next.png)

Filter an existing result set when the live query returns too much:

![Filtering results](docs/images/example-next-alt-1.png)

![Filtered results](docs/images/example-next-alt-2.png)

## Setup

Build from source:

```sh
cargo build --release
```

Install the release binary and shell completions:

```sh
./install
```

Generate a config from your libraries:

```sh
NEITH_LIBS="$HOME/neith/neith-lib:$HOME/neith/neith-devdocs/generated" neith config init
```

Build indexes and start Neith:

```sh
neith index
neith
```

Runtime dependencies:

- An editor command from `editor.command`, `$EDITOR`, or `vi`.
- `wl-copy`, `xclip`, `xsel`, `tmux`, or `clipboard.command` for clipboard copy.
- `man` and `col` for live man-page results.
- Optional: `bat` or `batcat` for syntax-highlighted previews.

See [Configuration](docs/configuration.md) for config fields and
[Installation](docs/installation.md) for installation details.

## Knowledge Libraries

Neith works with any number of Markdown library directories. Each library owns
its cache under `.neith-cache/neith/`, so indexes are local to the library.

Use a root `.neithignore` file to exclude library-owned files from indexing:

```gitignore
AGENTS.md
.neith-cache/
.agents/
```

Use `Ctrl-A` or `neith add <query...>` to create notes from the configured
library template. Use quick-copy regions in notes when a command or snippet
should be copied directly from the TUI.

See [Libraries](docs/libraries.md) for indexing and exclusion rules. See
[Quick-Copy](docs/quick-copy.md) for `copy_begin` wrappers.

## tmux Popup

Bind Neith to a tmux popup:

```sh
bind-key -r N run-shell "/path/to/neith/tmux_popup"
```

![tmux popup](docs/images/example-tmux.png)

From the popup, `Ctrl-O` opens the selected result in a new tmux split and
leaves a shell in that pane after the editor exits.

## Documentation

The root README is intentionally short. Detailed docs live in
[`docs/`](docs/):

| Doc | Covers |
| --- | --- |
| [Docs Index](docs/README.md) | Complete user and developer documentation map. |
| [Installation](docs/installation.md) | Build, install, uninstall, completions, and first run. |
| [Dependencies](docs/dependencies.md) | Build dependencies, runtime commands, optional integrations, and Rust crates. |
| [Configuration](docs/configuration.md) | Config file location, library precedence, editor, clipboard, preview, and prompt settings. |
| [Libraries](docs/libraries.md) | Library layout, indexing, cache ownership, `.neithignore`, source classification, and status checks. |
| [Indexing](docs/indexing.md) | Index commands, per-library cache layout, incremental updates, startup behavior, and status. |
| [Search](docs/search.md) | Query modes, result filters, library scope, live man pages, and result fields. |
| [Usage](docs/usage.md) | Commands, TUI keys, and JSON query output. |
| [Quick-Copy](docs/quick-copy.md) | `copy_begin` regions, command/output wrappers, prompt stripping, block chooser behavior, and clipboard backends. |
| [Note Creation](docs/note-creation.md) | CLI/TUI note creation, target library selection, path inference, and templates. |
| [Diagnostics](docs/diagnostics.md) | `status`, `healthcheck`, output fields, and exit codes. |
| [tmux](docs/tmux.md) | Popup helper and pane-open behavior. |
| [Developer Reference](docs/developer/README.md) | Code map, subsystem interfaces, dependencies, flows, and invariants. |

## Development

Run the test script:

```sh
./test
```

Contributor documentation lives in
[docs/developer/](docs/developer/README.md).
