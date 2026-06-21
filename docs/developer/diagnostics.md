# Developer Reference: Diagnostics

## Code

Primary file: `src/diagnostics.rs`.

Related files:

- `src/main.rs`: dispatches `status` and `healthcheck`.
- `src/config.rs`: loaded by `healthcheck`.
- `src/indexer.rs`: provides `library_status`.
- `src/action.rs`: provides `command_exists`.

## Public Types

| Type | Role |
| --- | --- |
| `CheckLevel` | `ok`, `warning`, or `failure`. |
| `Check` | One healthcheck row. |
| `HealthReport` | Collection of checks and exit-code logic. |
| `StatusRow` | User-facing library status row. |

## Public Functions

| Function | Role |
| --- | --- |
| `collect_status_rows` | Build status rows for configured libraries. |
| `healthcheck` | Run config, library, cache, index, and tool checks. |
| `render_healthcheck` | Render human-readable health table. |
| `render_status_table` | Render human-readable status table. |
| `render_index_table` | Render `neith index` stats. |
| `use_color_stdout` | Detect color support for stdout. |

## Dependencies

Internal:

- `src/config.rs`: config and library loading for healthcheck.
- `src/indexer.rs`: cache/index status and index stats.
- `src/library.rs`: library values.
- `src/action.rs`: external command lookup.

External crates:

- `serde`: JSON output.
- `anyhow`: status collection errors.

## Healthcheck Flow

1. Load `RuntimeConfig`.
2. Report config result.
3. Report library count.
4. Check each library path.
5. Warn on duplicate aliases.
6. Check cache/index state through `library_status`.
7. Check editor command.
8. Check clipboard backend.
9. Check `man` and `col`.

## Exit Codes

`HealthReport::exit_code`:

- `1` if any failure exists.
- `2` if warnings exist and no failures exist.
- `0` otherwise.

## Invariants

- JSON output should serialize the same data as table output.
- Warnings should not prevent normal use.
- Config load failure produces a report with one failure instead of panicking.
