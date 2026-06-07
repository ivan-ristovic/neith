# Neith Native

Native Rust implementation of Neith.

The binary is named `neith`. It searches configured Markdown knowledge
libraries through per-library Tantivy indexes stored under
`.neith-cache/neith/`.

## Build

```sh
cargo build --release
```

## Install

```sh
./install
```

The installer builds the release binary, symlinks it as `/usr/local/bin/neith`,
and installs bash and zsh completions.

## Uninstall

```sh
./uninstall
```

The uninstaller removes the `/usr/local/bin/neith` symlink when it points to
this checkout, plus the installed bash and zsh completion files.

## Run

```sh
NEITH_LIBS="/home/ivan/neith/neith-lib:/home/ivan/neith/neith-devdocs/generated" cargo run -- awk print 3rd column
```

Inside tmux:

```sh
bind-key -r N run-shell "/path/to/neith/tmux_popup"
```

Popup sizing can be adjusted with `NEITH_POPUP_WIDTH` and `NEITH_POPUP_HEIGHT`.

## Keys

- `Tab`: switch results/preview focus
- `Ctrl-K`: toggle exact/fuzzy query mode
- `Ctrl-R`: toggle fuzzy refine over the current result list
- `Ctrl-T`: cycle `all`, `names`, `content`, `man`
- `Ctrl-L`: cycle pinned libraries or open the library selector, including `all`
- `Ctrl-H`: open or close the help popup
- `Enter` in results: open the selected result in the editor
- `Enter` or `Space` in preview: anchor copy selection; press again to copy selected lines
- `v` in copy mode: move the selection anchor to the current line
- arrows: move result selection or preview cursor
- `j/k`, `PageUp`, `PageDown`: move/scroll preview
- `Ctrl-Q`: quit from any mode
- `Esc`: cancel mode/focus or quit from results

## Commands

```sh
neith index
neith index --rebuild
neith status
neith status --json
neith healthcheck
neith healthcheck --json
neith add awk "print selected fields"
neith completions bash
neith completions zsh
neith config init
neith json query awk print 3rd column
```
