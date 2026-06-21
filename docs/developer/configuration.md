# Developer Reference: Configuration

## Code

Primary file: `src/config.rs`.

Related files:

- `src/main.rs`: passes `--config` and `--libs` into config loading.
- `src/diagnostics.rs`: calls `RuntimeConfig::load` during healthcheck.
- `config-sample.toml`: tracked example config.

## Public Types

| Type | Role |
| --- | --- |
| `AppConfig` | Deserialized TOML root. |
| `LibraryConfig` | One `[[libraries]]` entry. |
| `EditorConfig` | Editor command and return behavior. |
| `ClipboardConfig` | Optional clipboard command. |
| `UiConfig` | Preview and prompt settings. |
| `PromptConfig` | Prompt separators and colors. |
| `PromptColorsConfig` | Named prompt colors. |
| `EditorReturn` | `exit` or `resume`. |
| `PreviewSyntax` | `auto`, `plain`, or `bat`. |
| `RuntimeConfig` | Resolved config plus concrete `Library` values. |

## Dependencies

Internal:

- `src/library.rs`: constructs resolved `Library` values and applies alias and
  pinned defaults.

External crates:

- `serde`: derives config serialization and deserialization.
- `toml`: parses and writes `config.toml`.
- `dirs`: resolves config and home directories.
- `anyhow`: reports config load and write errors with context.

## Load Flow

`RuntimeConfig::load(config_path, libs_arg)`:

1. Uses explicit `config_path` or `default_config_path`.
2. Reads TOML if the file exists; otherwise uses defaults.
3. Fills empty editor command from `$EDITOR` or `vi`.
4. Trims clipboard command.
5. Adds TOML libraries.
6. Adds `NEITH_LIBS`.
7. Adds `--libs`.
8. Deduplicates by canonical path when possible.
9. Drops missing or non-directory libraries.
10. Fails if no libraries remain.

## Config Init Flow

`RuntimeConfig::init_config(path, force, libs_arg)`:

1. Resolves output path.
2. Fails if the file exists and `force` is false.
3. Loads runtime config using the target path and runtime libraries.
4. Serializes resolved libraries and current config defaults to TOML.
5. Creates parent directories.
6. Writes the config.

## Interfaces

Called from:

- `main::run` for normal startup.
- `main::run` for `config init`.
- `diagnostics::healthcheck`.

Consumes:

- `Library::new`
- `dirs::config_dir`
- `dirs::home_dir`

Produces:

- `RuntimeConfig { path, app, libraries }`

## Invariants

- Config loading must not create files.
- `config init` is the only config writer.
- `~/` expansion only applies at the start of a path.
- Alias/pinned defaults are delegated to `Library::new`.
