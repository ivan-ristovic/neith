use super::*;

#[test]
fn marked_code_block_copies_block_body() {
    let text = r#"# Note

<!-- copy_begin -->
```bash
fd -e md
```
<!-- copy_end -->
"#;

    assert_eq!(
        extract(text),
        Ok(ExtractedCopy::Payload("fd -e md".to_string()))
    );
}

#[test]
fn marked_plain_text_copies_region_body() {
    let text = r#"# Note

<!-- copy_begin -->
Copy this text.
And this line.
<!-- copy_end -->
"#;

    assert_eq!(
        extract(text),
        Ok(ExtractedCopy::Payload(
            "Copy this text.\nAnd this line.".to_string()
        ))
    );
}

#[test]
fn marked_command_output_copies_selected_command_lines_and_strips_prompt() {
    let text = r#"# Note

<!-- copy_begin l=1 p=$ -->
```bash
$ echo 'foo'
foo
```
<!-- copy_end -->
"#;

    assert_eq!(
        extract(text),
        Ok(ExtractedCopy::Payload("echo 'foo'".to_string()))
    );
}

#[test]
fn marked_root_command_output_strips_hash_prompt() {
    let text = r#"# Note

<!-- copy_begin l=1 p=# -->
```bash
# systemctl restart nginx
```
<!-- copy_end -->
"#;

    assert_eq!(
        extract(text),
        Ok(ExtractedCopy::Payload(
            "systemctl restart nginx".to_string()
        ))
    );
}

#[test]
fn marked_multiline_command_output_copies_selected_command_lines() {
    let text = r#"# Note

<!-- copy_begin l=3 p=$ -->
```bash
$ cd /tmp
$ printf '%s\n' foo
$ pwd
/tmp
```
<!-- copy_end -->
"#;

    assert_eq!(
        extract(text),
        Ok(ExtractedCopy::Payload(
            "cd /tmp\nprintf '%s\\n' foo\npwd".to_string()
        ))
    );
}

#[test]
fn prompt_stripping_requires_prompt_followed_by_space() {
    let text = r#"# Note

<!-- copy_begin l=2 p=$ -->
```bash
  $ echo ok
$PATH
```
<!-- copy_end -->
"#;

    assert_eq!(
        extract(text),
        Ok(ExtractedCopy::Payload("  echo ok\n$PATH".to_string()))
    );
}

#[test]
fn invalid_copy_begin_attributes_are_errors() {
    assert_eq!(
        extract("<!-- copy_begin l=0 -->\ntext\n<!-- copy_end -->"),
        Err("copy_begin l must be a positive integer".to_string())
    );
    assert_eq!(
        extract("<!-- copy_begin p=> -->\ntext\n<!-- copy_end -->"),
        Err("copy_begin p must be $ or #".to_string())
    );
    assert_eq!(
        extract("<!-- copy_begin x=1 -->\ntext\n<!-- copy_end -->"),
        Err("unsupported copy_begin attribute: x=1".to_string())
    );
}

#[test]
fn markers_inside_code_blocks_are_ignored() {
    let text = r#"# Note

```text
<!-- copy_begin -->
not a marker
<!-- copy_end -->
```
"#;

    assert_eq!(
        extract(text),
        Ok(ExtractedCopy::Payload(
            "<!-- copy_begin -->\nnot a marker\n<!-- copy_end -->".to_string()
        ))
    );
}

#[test]
fn unterminated_region_is_an_error() {
    assert_eq!(
        extract("<!-- copy_begin -->\nfd -e md\n"),
        Err("copy_begin marker without copy_end marker".to_string())
    );
}

#[test]
fn note_without_region_or_code_block_is_an_error() {
    assert_eq!(
        extract("# Note\n\nNo payload here."),
        Err("no quick-copy region or code block found".to_string())
    );
}

#[test]
fn single_code_block_is_fallback_payload() {
    let text = r#"# Note

```bash
rg pattern
```
"#;

    assert_eq!(
        extract(text),
        Ok(ExtractedCopy::Payload("rg pattern".to_string()))
    );
}

#[test]
fn multiple_code_blocks_return_choices() {
    let text = r#"# Note

```bash
first
```

```python
second()
```
"#;

    assert_eq!(
        extract(text),
        Ok(ExtractedCopy::CodeBlocks(vec![
            CodeBlock {
                language: "bash".to_string(),
                body: "first".to_string(),
            },
            CodeBlock {
                language: "python".to_string(),
                body: "second()".to_string(),
            },
        ]))
    );
}
