# Search

Neith searches indexed library entries and live man-page results.

## Query Modes

`Ctrl-X` toggles query mode:

| Mode | Behavior |
| --- | --- |
| `fuzzy` | Default mode. Uses indexed search and a fuzzy catalog fallback. |
| `exact` | Uses indexed search and verifies regex-like queries against result text. |

Neith normalizes ordinal query terms. For example, `3rd`, `nth`, and `$3` are
normalized to `selected`, which helps queries such as `awk print 3rd column`
match notes written with more generic wording.

## Result Filters

`Ctrl-T` cycles result type filters:

| Filter | Searches |
| --- | --- |
| `all` | All result types. |
| `names` | Titles, paths, and name-oriented results. |
| `content` | Body and excerpt content. |
| `man` | Indexed man entries and live man-page results. |

## Library Scope

`Ctrl-L` cycles pinned libraries. If a library picker opens, select a library
or the `all` row.

## Live Man Pages

For `all` and `man` filters, Neith treats the first query token as a possible
man-page title.

Supported section syntax:

```text
printf(3) format
3 printf format
```

Neith calls `man -wa`, renders matching pages through `man -l` and `col -b`,
and caches the rendered text under `${XDG_CACHE_HOME:-~/.cache}/neith/man/`.

## Result Display

Each result includes:

- source kind
- library alias
- relative path
- optional line number
- snippet
- rank reason

`Enter` opens the selected result in the configured editor.

Related developer docs:

- [Developer Search](developer/search.md)
- [Developer TUI](developer/tui.md)
