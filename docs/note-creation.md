# Note Creation

Neith can create notes from the CLI or TUI.

## CLI

```sh
neith add bash enable strict mode
```

The first argument is the folder. The remaining words form the note title, so
this example opens `<library>/bash/enable-strict-mode.md`. The title must
contain a letter or number. An existing note opens directly.

Installed Bash and Zsh completions suggest existing library folders for the
first argument. Press Tab after `neith add ` to see them. Nested folders are
shown as relative paths, such as `shell/awk`.

To choose an existing folder with `fzf` instead:

```sh
neith add --pick-folder enable strict mode
```

`fzf` opens before the editor. Its selected folder is paired with the title
arguments. Cancelling the picker creates nothing.

## TUI

Press `Ctrl-A` to open the add prompt. Enter a folder followed by a title, such
as `bash enable strict mode`. The current query fills this input. Press
`Ctrl-P` to pick an existing folder with `fzf`; the remaining title stays in
place. The destination appears in the bottom row. Press `Tab` to edit the path
when needed, then `Enter` to open the draft in your editor. Press `Esc` to
cancel the prompt.

For a new note, the editor works on a temporary draft. Saving it adds the note
to the library. Closing the editor without saving discards the draft, so no
`untitled.md` or empty category directory is left behind.

## Target Library

With the TUI scoped to a pinned library, new notes go to that library. In the
`all` scope and for CLI `neith add`, Neith chooses:

1. The first library with alias `neith-lib`.
2. Otherwise, the first configured library.

## Path Inference

For input `awk print 3rd column`:

1. The first word, `awk`, chooses the folder.
2. If `<library>/awk` exists, Neith uses that directory.
3. Otherwise Neith creates the sanitized folder when the note is saved.
4. The remaining words become the filename slug.

Example destination:

```text
<library>/awk/print-3rd-column.md
```

Existing files are left unchanged.
Quote a folder name that contains spaces in the TUI, such as
`"shell tools" safe aliases`. In the CLI, quote it for your shell:
`neith add "shell tools" safe aliases`.

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

When no template exists, Neith fills the draft with a default title, task text,
starter quick-copy region, and references section.

Related developer docs:

- [Developer Note Creation](developer/note-creation.md)
