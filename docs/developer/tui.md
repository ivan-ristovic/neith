# Developer Reference: TUI And Usage

## Code

Primary file: `src/tui.rs`.

Related files:

- `src/search.rs`: search engine and results.
- `src/query.rs`: query mode, source filter, library scope.
- `src/action.rs`: copy, editor, tmux pane open.
- `src/note.rs`: add-entry path and file creation.
- `src/quick_copy.rs`: quick-copy extraction.
- `src/config.rs`: UI, editor, clipboard settings.

## Main Types

| Type | Role |
| --- | --- |
| `App` | TUI state machine. |
| `Focus` | Active UI mode: results, preview, library selector, add entry, quick-copy, help. |
| `PreviewState` | Preview lines, cursor, scroll, copy selection. |
| `QuickCopyState` | Code-block chooser state. |

## Dependencies

Internal:

- `src/search.rs` and `src/query.rs`: request construction and result data.
- `src/action.rs`: clipboard, editor, and tmux pane actions.
- `src/note.rs`: add-entry path inference and file creation.
- `src/quick_copy.rs`: payload extraction and code-block choices.
- `src/config.rs`: UI, editor, and clipboard settings.

External crates:

- `crossterm`: terminal events, raw mode, alternate screen, and mouse capture.
- `ratatui`: layout and rendering.
- `ansi-to-tui`: conversion of `bat` ANSI output into preview spans.

## Startup

`tui::run(config, engine, initial_query)`:

1. Initializes terminal raw mode and mouse capture.
2. Creates `App`.
3. Runs the event loop.
4. Restores terminal state on exit.

## Search Updates

The TUI builds `SearchRequest` from:

- query text
- result filter
- match mode
- library scope
- result limit

Search is refreshed when query text, filter, mode, or library scope changes.

## Focus Modes

| Focus | Purpose |
| --- | --- |
| `Results` | Edit query and select results. |
| `Preview` | Scroll preview and select lines to copy. |
| `LibrarySelector` | Choose one library or all libraries. |
| `AddEntry` | Edit inferred note path. |
| `QuickCopy` | Choose a code block from a multi-block note. |
| `Help` | Show key reference. |

## Key Handling

Control keys are handled before focus-specific keys. Focus-specific handlers
then process normal keys.

Important handlers:

- `handle_key`
- `handle_results_key`
- `handle_preview_key`
- `handle_library_selector_key`
- `handle_add_entry_key`
- `handle_quick_copy_key`
- `handle_help_key`

## Preview Rendering

Preview source:

1. `bat`/`batcat` when enabled and available.
2. Plain text fallback.

`ui.preview_cursor_percent` controls cursor placement during scrolling.

## Editor Return

Normal result opens request editor work from the event loop. If
`editor.return_behavior = "resume"`, the TUI refreshes indexes/readers and
continues after editor exit. If set to `exit`, Neith exits.

## Invariants

- Terminal state must be restored after `tui::run`.
- Modal focus states should be cancellable with `Esc`.
- Preview copy and quick-copy must report copy failures in status text.
- Help text should match key handlers.
