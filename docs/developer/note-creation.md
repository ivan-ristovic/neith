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
| `create_note` | Create parent dirs and write body if file does not exist. |
| `slugify` | Convert query text to lower-kebab-case path component. |

## Dependencies

Internal:

- `src/library.rs`: provides configured libraries and aliases.
- `src/main.rs`: dispatches the `neith add` command.
- `src/tui.rs`: uses the same logic from the add-entry prompt.

External crates:

- `anyhow`: missing-library and filesystem errors.

## Target Library Selection

`infer_note_path` uses:

1. First library with alias `neith-lib`.
2. Otherwise first configured library.

It fails if there are no libraries.

## Path Logic

Inputs:

- configured libraries
- query string

Output:

- destination path

The first query word is the category candidate. If a matching directory exists
under the target library, Neith uses it. Otherwise it uses a sanitized directory
from the first word.

The filename is the slugified full query, or `untitled.md` for an empty query.

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
