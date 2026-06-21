# Note Creation

Neith can create notes from the CLI or TUI.

## CLI

```sh
neith add bash enable strict mode
```

This creates or opens a Markdown file inferred from the query and launches the
configured editor.

## TUI

Press `Ctrl-A` to open the add prompt. Edit the inferred path, press `Enter` to
create or open it, or press `Esc` to cancel.

## Target Library

Neith creates notes in:

1. The first library with alias `neith-lib`.
2. Otherwise, the first configured library.

## Path Inference

For query `awk print 3rd column`:

1. The first word, `awk`, is used as the category candidate.
2. If `<library>/awk` exists, Neith uses that directory.
3. Otherwise Neith creates or uses a directory from the sanitized first word.
4. The full query becomes the filename slug.

Example destination:

```text
<library>/awk/awk-print-3rd-column.md
```

Existing files are left unchanged.

## Templates

Template lookup starts at the destination directory and walks upward looking for
`.neith-note-template.md`.

Supported placeholders:

| Placeholder | Value |
| --- | --- |
| `{{TITLE}}` | Title generated from the destination filename. |
| `{{QUERY}}` | Original add query. |
| `{{SLUG}}` | Destination filename stem. |
| `{{PATH}}` | Destination path. |

When no template exists, Neith writes a default note with a title, task text, a
starter quick-copy region, and a references section.

Related developer docs:

- [Developer Note Creation](developer/note-creation.md)
