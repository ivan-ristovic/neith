# Developer Reference: Quick-Copy

## Code

Primary file: `src/quick_copy.rs`.

Related files:

- `src/tui.rs`: invokes quick-copy on `Ctrl-C`.
- `src/action.rs`: copies extracted text to clipboard.
- `src/note.rs`: default note template includes a copy region.

## Public Types

| Type | Role |
| --- | --- |
| `CodeBlock` | Language and body for one fenced code block. |
| `ExtractedCopy` | Either a payload or a list of code blocks for chooser UI. |

## Public Functions

| Function | Role |
| --- | --- |
| `extract` | Extract quick-copy payload or code-block choices from Markdown text. |
| `first_non_empty_line` | Helper for preview labels. |

## Dependencies

Internal:

- `src/tui.rs`: calls extraction and presents chooser state.
- `src/action.rs`: copies the extracted payload to the clipboard.
- `src/note.rs`: emits the default copy region in new notes.

External crates: none.

## Extraction Flow

`extract`:

1. Calls `extract_copy_region`.
2. If a copy region exists, extracts exactly one code block body or region text.
3. Applies `CopyOptions`.
4. Returns payload or empty-region error.
5. If no region exists, returns the only code block as payload.
6. If multiple code blocks exist, returns `CodeBlocks`.
7. If no payload exists, returns an error.

## Marker Parsing

Supported marker:

```md
<!-- copy_begin l=1 p=$ -->
```

Attributes:

- `l=N`: positive integer, line limit.
- `p=$`: strip `$ ` prompt.
- `p=#`: strip `# ` prompt.

Unsupported attributes are errors. Duplicate attributes are errors.

Markers inside fenced code blocks are ignored by the region parser.

## Invariants

- Prompt stripping requires prompt followed by a space.
- Prompt stripping preserves indentation before the prompt.
- Line limiting happens before prompt stripping.
- Unterminated copy regions are errors.
- Empty payloads are errors.
