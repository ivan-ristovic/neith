# Developer Reference: tmux

## Code

Files:

- `tmux_popup`
- `src/action.rs`
- `src/tui.rs`

## Dependencies

Internal:

- `tmux_popup`: records popup context and launches Neith.
- `src/action.rs`: opens selected results in tmux panes.
- `src/tui.rs`: calls pane open logic for `Ctrl-O`.

External commands:

- `tmux`
- configured editor command
- `${SHELL:-sh}` for the shell left in the split pane

## Popup Script

`tmux_popup`:

1. Resolves repository root.
2. Uses `NEITH_BIN` or `target/release/neith`.
3. Runs `release` when the binary is missing.
4. If outside tmux, execs Neith directly.
5. Records the current pane id in `NEITH_TMUX_TARGET_PANE`.
6. Calls `tmux display-popup`.

Environment variables:

- `NEITH_BIN`
- `NEITH_POPUP_WIDTH`
- `NEITH_POPUP_HEIGHT`
- `NEITH_POPUP_TITLE`

## Pane Open

`action::open_editor_in_tmux_pane`:

1. Requires `TMUX`.
2. Requires `tmux` in `PATH`.
3. Finds target pane from `NEITH_TMUX_TARGET_PANE` or `TMUX_PANE`.
4. Queries target pane current path.
5. Queries pane width/height.
6. Chooses split flag.
7. Builds a shell-quoted editor command.
8. Runs `tmux split-window`.

Split flag:

- `-v` for wide panes.
- `-h` for tall or square panes.

## Invariants

- Target pane lookup must work from popup sessions.
- The opened editor command must quote paths and editor args.
- The pane command must end with `exec "${SHELL:-sh}"`.
