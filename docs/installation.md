# Installation

Neith is built from this Rust workspace and installed with repository scripts.

## Build

Build the release binary:

```sh
cargo build --release
```

The binary is written to:

```text
target/release/neith
```

## Install

Install the release binary and shell completions:

```sh
./install
```

The installer:

- Builds the release binary.
- Creates `/usr/local/bin`.
- Symlinks `target/release/neith` to `/usr/local/bin/neith`.
- Writes bash completions to `/usr/share/bash-completion/completions/neith`.
- Writes zsh completions to `/usr/local/share/zsh/site-functions/_neith`.

The script uses `sudo` for system paths.

## Uninstall

Remove installed files:

```sh
./uninstall
```

The uninstaller removes shell completions. It removes `/usr/local/bin/neith`
only when that path is a symlink to this checkout's release binary.

## Manual Completion Installation

Generate completions directly:

```sh
neith completions bash
neith completions zsh
```

Example manual install paths:

```sh
neith completions bash > ~/.local/share/bash-completion/completions/neith
neith completions zsh > ~/.local/share/zsh/site-functions/_neith
```

## First Run

Create a config from explicit libraries:

```sh
NEITH_LIBS="$HOME/neith/neith-lib:$HOME/neith/neith-devdocs/generated" neith config init
```

Build indexes:

```sh
neith index
```

Start the TUI:

```sh
neith
```

Related docs:

- [Dependencies](dependencies.md)
- [Configuration](configuration.md)
- [Indexing](indexing.md)
- [Usage](usage.md)
