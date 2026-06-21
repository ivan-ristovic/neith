# Quick-Copy

Quick-copy copies the selected note payload with `Ctrl-C` in the TUI.

## Extraction Order

Neith extracts quick-copy payloads in this order:

1. Use the first complete `<!-- copy_begin -->` / `<!-- copy_end -->` region.
2. If the region contains exactly one fenced code block, copy only the block body.
3. If no region exists and the note has one code block, copy that block body.
4. If no region exists and the note has multiple code blocks, open a chooser.

The chooser supports `1-9`, `Up/Down`, `j/k`, `Enter`, `Space`, `Esc`, and
`Tab`.

## Basic Region

````md
<!-- copy_begin -->
```bash
fd -e md
```
<!-- copy_end -->
````

Copied payload:

```bash
fd -e md
```

## Command Output Preview

Use `l=N` and `p=$` when a code block includes shell prompt and output:

````md
<!-- copy_begin l=1 p=$ -->
```bash
$ echo 'foo'
foo
```
<!-- copy_end -->
````

Copied payload:

```bash
echo 'foo'
```

## Multi-Line Commands

````md
<!-- copy_begin l=2 p=$ -->
```bash
$ printf '%s\n' foo \
  bar
foo
bar
```
<!-- copy_end -->
````

Copied payload:

```bash
printf '%s\n' foo \
  bar
```

## Root Prompts

Use `p=#` for root-shell examples:

````md
<!-- copy_begin l=1 p=# -->
```bash
# systemctl restart nginx
```
<!-- copy_end -->
````

`p=` accepts only `$` and `#`. Prompt stripping permits optional indentation and
requires the prompt character to be followed by a space. `$HOME` is not stripped.

## Clipboard Backends

Preview copy and quick-copy share clipboard behavior:

1. Use `clipboard.command` when configured.
2. Try `wl-copy`.
3. Try `xclip -sel clip`.
4. Try `xsel --clipboard --input`.
5. Use `tmux set-buffer` when inside tmux.

Related developer docs:

- [Developer Quick-Copy](developer/quick-copy.md)
- [Developer Dependencies](developer/dependencies.md)
