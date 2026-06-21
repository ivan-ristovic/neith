# Libraries

A Neith library is a directory tree containing Markdown files. Neith indexes
Markdown notes, generated DevDocs pages, and generated man-page entries.

## Layout

Example:

```text
neith-lib/
  awk/print-selected-fields.md
  git/reset-a-reset.md
  unix/list-files.md
  .neithignore
  .neith-cache/neith/
```

Each library owns its cache under:

```text
<library>/.neith-cache/neith/
```

## Discovery

Neith indexes files ending in `.md`.

The walker always skips:

```text
.git
.neith-cache
target
.cache
```

Symlink following is disabled.

## `.neithignore`

Each library can define exclusions in a root `.neithignore` file.

Example:

```gitignore
AGENTS.md
.neith-note-template.md
.agents/
.codex/
.neith-cache/
```

Rules:

- Blank lines are ignored.
- Lines starting with `#` are comments.
- Plain entries match exact paths relative to the library root.
- Entries ending in `/` match directory prefixes.

Ignored files do not appear in indexes, search results, or status counts.

## Aliases

Each library has an alias used in result display and filtering. Set it
explicitly in config:

```toml
[[libraries]]
path = "~/neith/neith-lib"
alias = "neith-lib"
```

When no alias is configured, Neith infers one:

| Path pattern | Inferred alias |
| --- | --- |
| Contains `neith-devdocs/generated` | `devdocs` |
| Ends with `neith-lib` | `neith-lib` |
| Ends with `ol-docs` | `ol-docs` |
| Other path | Last useful path component, or `library` |

Default pinned aliases are `neith-lib`, `devdocs`, and `ol-docs`.

## Source Kinds

Every entry is classified as one source kind:

| Source kind | Meaning |
| --- | --- |
| `note` | Normal Markdown note. |
| `devdocs` | Generated DevDocs page. |
| `man` | Man-page entry. |

`Ctrl-T` cycles result filters for these source kinds.

## Note Templates

`neith add` and `Ctrl-A` can create notes from a `.neith-note-template.md`.
Template lookup starts at the destination directory and walks upward. Supported
placeholders are documented in [Note Creation](note-creation.md).

Related docs:

- [Configuration](configuration.md)
- [Indexing](indexing.md)
- [Developer Libraries](developer/libraries.md)
