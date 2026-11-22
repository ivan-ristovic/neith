# Developer Reference: Note Creation

## Code

Primary file: `src/note.rs`.

Related files:

- `src/main.rs`: handles `neith add`.
- `src/tui.rs`: handles `Ctrl-A` add prompt.
- `src/library.rs`: provides library aliases and paths.

## Public Functions

| Function | Role |
| --- | --- |
| `infer_note_path` | Choose target library/category and filename. |
| `infer_note_path_in_library` | Infer a path inside the TUI's selected library. |
| `infer_note_path_from_args` | Parse CLI folder and title arguments. |
| `list_note_folders` | List existing relative directories for completion and `fzf`. |
| `pick_note_folder` | Run `fzf` against the target library's directories. |
| `prepare_note_edit` | Open an existing note or stage a new template in a temporary Markdown file. |
| `NoteEdit::finish` | Discard an unsaved draft or create the destination after a successful editor save. |
| `validate_note_title` | Reject empty or punctuation-only titles. |
| `create_note` | Create parent dirs and write body if file does not exist. |
| `slugify` | Convert query text to lower-kebab-case path component. |

## Dependencies

Internal:

- `src/library.rs`: provides configured libraries and aliases.
- `src/main.rs`: dispatches the `neith add` command.
- `src/tui.rs`: uses the same logic from the add-entry prompt.

External crates:

- `anyhow`: missing-library and filesystem errors.
- `tempfile`: temporary editor drafts and cleanup.

## Target Library Selection

`infer_note_path` uses:

1. First library with alias `neith-lib`.
2. Otherwise first configured library.

It fails if there are no libraries.
The TUI uses `infer_note_path_in_library` when its scope names a pinned library.

## Path Logic

Inputs:

- configured libraries
- query string

Output:

- destination path

The first token selects a relative folder. The remaining text is the title.
If the folder exists, Neith uses its exact name. Otherwise it sanitizes each
path component and creates the folder when the draft is saved. Traversal and
symlinks that resolve outside the library are rejected.

The filename is the slugified title. The CLI and TUI require a nonempty title
after the folder before preparing an editor draft. The TUI accepts a quoted
folder token for names with spaces; the CLI keeps the first shell argument as
the folder.

## Folder Picker And Completion

`list_note_folders` walks the selected library, skips hidden directories and
`target`, and returns sorted relative paths. The TUI suspends its terminal and
passes this list to `fzf` on `Ctrl-P`. CLI `--pick-folder` uses the same picker.
The hidden `__complete-folders` command supplies current directories to the
generated Bash and Zsh completion hooks.

## Editor Drafts

`prepare_note_edit` leaves existing files alone. For a new file it writes the
rendered template to a temporary `.md` draft. `NoteEdit::finish` checks whether
the editor saved the draft. On a successful save, it creates the destination
with `create_new`, so an existing file cannot be overwritten. An unchanged
editor session discards the draft. Destination directories are created only
when the draft is saved. If the editor fails after saving or committing the
draft fails, Neith keeps the temporary file and reports its recovery path.

## Template Rendering

Private helpers:

- `render_note_body`
- `find_note_template`
- `default_note_template`
- `title_from_slug`

Template lookup walks upward from the destination directory. The first
`.neith-note-template.md` wins.

## Invariants

- Existing files are not overwritten.
- Parent directories are created before writing.
- The default template includes a quick-copy region and references section.
