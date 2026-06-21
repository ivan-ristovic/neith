# Indexing

Neith uses Tantivy indexes stored inside each configured library.

## Commands

Build or update indexes:

```sh
neith index
```

Remove and rebuild indexes:

```sh
neith index --rebuild
```

Rebuild before opening the TUI or running a JSON query:

```sh
neith --rebuild
neith --rebuild json query awk fields
```

## Cache Layout

Each library cache has this shape:

```text
<library>/.neith-cache/neith/
  tantivy/
    meta.json
    ...
  manifest.json
  catalog.json
```

`tantivy/` stores the full-text index. `manifest.json` stores file signatures
for incremental updates. `catalog.json` stores lightweight metadata used for
empty-query results, man-page matching, and fuzzy fallback.

Live man-page rendering uses a user cache:

```text
${XDG_CACHE_HOME:-~/.cache}/neith/man/
```

## Incremental Updates

During `neith index`, Neith:

1. Discovers current Markdown entries.
2. Loads the previous manifest.
3. Deletes entries whose paths disappeared.
4. Reindexes entries whose signatures changed.
5. Leaves unchanged entries in place.
6. Writes a new manifest and catalog.

A file is unchanged only when every signature field matches:

- absolute path
- relative path
- title
- excerpt
- source kind
- size
- modified time
- content hash

## Startup Behavior

The TUI and JSON query command ensure usable indexes before searching. If an
index is missing, Neith builds it. The TUI also starts a background incremental
refresh after opening.

## Status

Show index state:

```sh
neith status
neith status --json
```

Healthcheck reports missing or stale indexes:

```sh
neith healthcheck
```

Related developer docs:

- [Developer Indexing](developer/indexing.md)
