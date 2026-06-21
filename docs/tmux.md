# tmux

Neith has two tmux integrations:

- `tmux_popup` opens Neith in a popup.
- `Ctrl-O` opens the selected result in a new tmux pane.

## Popup

Bind the helper script:

```sh
bind-key -r N run-shell "/path/to/neith/tmux_popup"
```

`tmux_popup` uses these environment variables:

| Variable | Default | Meaning |
| --- | --- | --- |
| `NEITH_BIN` | `<repo>/target/release/neith` | Binary to execute. |
| `NEITH_POPUP_WIDTH` | `60%` | Popup width. |
| `NEITH_POPUP_HEIGHT` | `80%` | Popup height. |
| `NEITH_POPUP_TITLE` | `Neith` | Popup title. |

If the release binary is missing, the script runs `release`. If the script is
called outside tmux, it executes Neith directly.

## Open In Pane

Inside Neith, `Ctrl-O` opens the selected result in a new split and exits the
TUI. Neith targets the pane recorded by `NEITH_TMUX_TARGET_PANE`, falling back
to `TMUX_PANE`.

Split direction is based on the target pane size:

- Wide panes use a vertical split.
- Tall or square panes use a horizontal split.

The editor command ends with `exec "${SHELL:-sh}"`, leaving a shell in the pane
after the editor exits.

Related developer docs:

- [Developer tmux](developer/tmux.md)
