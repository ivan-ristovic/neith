# Developer Reference: Installation

## Files

| File | Role |
| --- | --- |
| `install` | Builds release binary and installs binary/completions. |
| `uninstall` | Removes installed symlink and completions. |
| `tmux_popup` | Popup wrapper. |
| `release` | Release build helper used by `tmux_popup` when binary is missing. |
| `src/cli.rs` | Completion shell enum. |
| `src/main.rs` | Completion generation dispatch. |

## Dependencies

Tools used by scripts:

- `cargo`
- `sudo`
- `ln`
- shell redirection for completion output

Rust interfaces:

- `clap::CommandFactory`
- `clap_complete::generate`

## Install Script

`install`:

1. Resolves the repository directory.
2. Runs `cargo build --release`.
3. Symlinks `target/release/neith` to `/usr/local/bin/neith`.
4. Generates bash completions.
5. Generates zsh completions.

Completion generation calls the compiled binary:

```sh
target/release/neith completions bash
target/release/neith completions zsh
```

## Uninstall Script

`uninstall` removes `/usr/local/bin/neith` only when it is a symlink to this
checkout's release binary. This avoids deleting another installation.

It always removes the completion files written by `install`.

## Completion Interfaces

Code path:

- `cli::Command::Completions`
- `cli::CompletionShell`
- `main::print_completions`

## Invariants

- Completion output must be generated from the current `Cli` definition.
- Install and uninstall paths must stay in sync.
- `install` assumes `sudo` is available for system paths.
