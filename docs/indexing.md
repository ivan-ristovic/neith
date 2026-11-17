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
empty-query results, man-page matching, fuzzy fallback, and regex filtering.

Live man-page rendering uses a user cache:

```text
${XDG_CACHE_HOME:-~/.cache}/neith/man/
```

## Incremental Updates

During `neith index`, Neith:

1. Discovers current Markdown entries.
2. Loads the previous manifest.
3. Compares current entries with saved signatures.
4. Opens the index writer only when documents were added, changed, or removed.
5. Writes the manifest and catalog when their contents changed. An unchanged
   index run leaves both files in place.

A file is unchanged only when every signature field matches:

- absolute path
- relative path
- title
- excerpt
- source kind
- size
- modified time
- content hash

The catalog also stores an optional search filter for each entry. Existing
caches are upgraded during the next index run without rebuilding their Tantivy
documents.

## Startup Behavior

The TUI and JSON query command ensure usable indexes before searching. If an
index is missing, Neith builds it. JSON queries also refresh existing indexes
before searching. The TUI starts a background incremental refresh after opening
and reloads its results when that refresh completes.

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
