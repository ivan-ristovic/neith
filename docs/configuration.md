# Configuration

Neith reads TOML config from:

```text
${XDG_CONFIG_HOME:-~/.config}/neith/config.toml
```

Use `--config PATH` to load another config file for one invocation.

## Minimal Config

```toml
[[libraries]]
path = "~/neith/neith-lib"
alias = "neith-lib"
pinned = true

[editor]
command = "nvim"
return_behavior = "resume"
```

## Complete Example

```toml
[[libraries]]
path = "~/neith/neith-lib"
alias = "neith-lib"
pinned = true

[[libraries]]
path = "~/neith/neith-devdocs/generated"
alias = "devdocs"
pinned = true

[editor]
command = "nvim"
return_behavior = "resume"

[clipboard]
command = "xclip -sel clip"

[ui]
preview_cursor_percent = 50
preview_syntax = "auto"
preview_bat_args = []

[ui.prompt]
separator = ":"
right_separator = ">"

[ui.prompt.colors]
fuzzy = "cyan"
exact = "red"
scope = "blue"
filter = "green"
separator = "dark-gray"
marker = "dark-gray"
query = "white"
add = "cyan"
```

See [`../config-sample.toml`](../config-sample.toml) for a tracked sample.

## Library Precedence

Libraries are appended in this order:

1. TOML `[[libraries]]`.
2. `NEITH_LIBS`, split on `:`.
3. `--libs`, split on `:`.

After appending, Neith deduplicates libraries by canonical path when possible.
Missing paths and non-directory paths are dropped. Startup fails if no library
remains.

`~/` is expanded at the start of a path. Shell variables such as `$HOME` are not
expanded inside TOML strings.

## Config Fields

| Field | Type | Behavior |
| --- | --- | --- |
| `libraries[].path` | path | Library root. |
| `libraries[].alias` | string, optional | Display and filter name. Inferred when omitted. |
| `libraries[].pinned` | bool, optional | Adds the library to the fast `Ctrl-L` scope cycle. |
| `editor.command` | string | Editor command. Defaults to `$EDITOR`, then `vi`. |
| `editor.return_behavior` | `exit` or `resume` | Whether the TUI exits or resumes after editor exit. |
| `clipboard.command` | string | Custom clipboard command. Neith writes copied text to stdin. |
| `ui.preview_cursor_percent` | integer | Preferred preview cursor position. `0` top, `50` middle, `100` bottom. |
| `ui.preview_syntax` | `auto`, `plain`, or `bat` | Preview syntax behavior. |
| `ui.preview_bat_args` | string array | Extra args passed to `bat` after Neith defaults and before the file path. |
| `ui.prompt.separator` | string | Separator between mode, scope, and filter. |
| `ui.prompt.right_separator` | string | Separator before query text. |
| `ui.prompt.colors.*` | color names | Prompt color settings. |

Supported color names are `black`, `red`, `green`, `yellow`, `blue`,
`magenta`, `cyan`, `gray`, `dark-gray`, `white`, `reset`, and `default`.

## Generate Config

Generate a config from resolved runtime libraries:

```sh
NEITH_LIBS="$HOME/neith/neith-lib:$HOME/neith/neith-devdocs/generated" neith config init
```

Overwrite an existing config:

```sh
neith config init --force
```

Related developer docs:

- [Developer Configuration](developer/configuration.md)
