# Diagnostics

Neith provides `status` and `healthcheck` commands for setup inspection and
automation.

## Status

```sh
neith status
neith status --json
```

Status output reports one row per library:

| Column | Meaning |
| --- | --- |
| `alias` | Library alias. |
| `kind` | `note`, `devdocs`, `man`, `mixed`, or `empty`. |
| `files` | Current discovered Markdown files. |
| `indexed` | Files recorded in the manifest. |
| `stale` | Files whose signatures differ from the manifest. |
| `cache` | Cache size, or `missing`. |
| `index` | `ok`, `stale`, or `missing`. |

## Healthcheck

```sh
neith healthcheck
neith healthcheck --json
```

Healthcheck validates:

- config loading
- configured libraries
- duplicate aliases
- index/cache state
- editor command
- clipboard backend
- `man`
- `col`

Exit codes:

| Code | Meaning |
| --- | --- |
| `0` | All checks ok. |
| `2` | One or more warnings, no failures. |
| `1` | One or more failures. |

Related developer docs:

- [Developer Diagnostics](developer/diagnostics.md)
