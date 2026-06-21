const COPY_BEGIN_MARKER: &str = "<!-- copy_begin -->";
const COPY_END_MARKER: &str = "<!-- copy_end -->";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeBlock {
    pub language: String,
    pub body: String,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExtractedCopy {
    Payload(String),
    CodeBlocks(Vec<CodeBlock>),
}

#[derive(Clone, Debug)]
struct MarkdownFence {
    marker: char,
    len: usize,
    info: String,
}

pub fn extract(text: &str) -> Result<ExtractedCopy, String> {
    if let Some(region) = extract_copy_region(text)? {
        let blocks = code_blocks(&region);
        let payload = if blocks.len() == 1 {
            blocks[0].body.clone()
        } else {
            trim_blank_line_edges(region.lines().map(str::to_string).collect())
        };
        if payload.trim().is_empty() {
            return Err("quick-copy region is empty".to_string());
        }
        return Ok(ExtractedCopy::Payload(payload));
    }

    let blocks = code_blocks(text);
    match blocks.len() {
        0 => Err("no quick-copy region or code block found".to_string()),
        1 => Ok(ExtractedCopy::Payload(blocks[0].body.clone())),
        _ => Ok(ExtractedCopy::CodeBlocks(blocks)),
    }
}

pub fn first_non_empty_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|line| !line.is_empty())
}

fn extract_copy_region(text: &str) -> Result<Option<String>, String> {
    let mut in_fence: Option<MarkdownFence> = None;
    let mut collecting = false;
    let mut region = Vec::new();

    for line in text.lines() {
        if let Some(fence) = &in_fence {
            if collecting {
                region.push(line.to_string());
            }
            if fence.closes(line) {
                in_fence = None;
            }
            continue;
        }

        let trimmed = line.trim();
        if collecting && trimmed == COPY_END_MARKER {
            let region = trim_blank_line_edges(region);
            if region.trim().is_empty() {
                return Err("quick-copy region is empty".to_string());
            }
            return Ok(Some(region));
        }
        if !collecting && trimmed == COPY_BEGIN_MARKER {
            collecting = true;
            region.clear();
            continue;
        }

        if let Some(fence) = MarkdownFence::opening(line) {
            if collecting {
                region.push(line.to_string());
            }
            in_fence = Some(fence);
            continue;
        }

        if collecting {
            region.push(line.to_string());
        }
    }

    if collecting {
        Err("copy_begin marker without copy_end marker".to_string())
    } else {
        Ok(None)
    }
}

fn code_blocks(text: &str) -> Vec<CodeBlock> {
    let mut blocks = Vec::new();
    let mut in_fence: Option<MarkdownFence> = None;
    let mut body = Vec::new();

    for line in text.lines() {
        if let Some(fence) = &in_fence {
            if fence.closes(line) {
                blocks.push(CodeBlock {
                    language: fence.language(),
                    body: trim_blank_line_edges(body),
                });
                body = Vec::new();
                in_fence = None;
            } else {
                body.push(line.to_string());
            }
            continue;
        }

        if let Some(fence) = MarkdownFence::opening(line) {
            in_fence = Some(fence);
        }
    }

    blocks
}

impl MarkdownFence {
    fn opening(line: &str) -> Option<Self> {
        let trimmed = line.trim_start();
        let marker = trimmed.chars().next()?;
        if marker != '`' && marker != '~' {
            return None;
        }
        let len = trimmed.chars().take_while(|ch| *ch == marker).count();
        if len < 3 {
            return None;
        }
        let info = trimmed[len..].trim().to_string();
        Some(Self { marker, len, info })
    }

    fn closes(&self, line: &str) -> bool {
        let trimmed = line.trim_start();
        let len = trimmed.chars().take_while(|ch| *ch == self.marker).count();
        len >= self.len && trimmed[len..].trim().is_empty()
    }

    fn language(&self) -> String {
        self.info
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_matches(|ch| matches!(ch, '{' | '}' | '.'))
            .to_string()
    }
}

fn trim_blank_line_edges(mut lines: Vec<String>) -> String {
    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
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
}
