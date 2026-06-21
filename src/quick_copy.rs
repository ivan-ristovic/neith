const COPY_BEGIN_PREFIX: &str = "<!-- copy_begin";
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CopyOptions {
    lines: Option<usize>,
    prompt: Option<char>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CopyRegion {
    body: String,
    options: CopyOptions,
}

pub fn extract(text: &str) -> Result<ExtractedCopy, String> {
    if let Some(region) = extract_copy_region(text)? {
        let blocks = code_blocks(&region.body);
        let payload = if blocks.len() == 1 {
            blocks[0].body.clone()
        } else {
            trim_blank_line_edges(region.body.lines().map(str::to_string).collect())
        };
        let payload = region.options.apply(&payload);
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

fn extract_copy_region(text: &str) -> Result<Option<CopyRegion>, String> {
    let mut in_fence: Option<MarkdownFence> = None;
    let mut options: Option<CopyOptions> = None;
    let mut region = Vec::new();

    for line in text.lines() {
        if let Some(fence) = &in_fence {
            if options.is_some() {
                region.push(line.to_string());
            }
            if fence.closes(line) {
                in_fence = None;
            }
            continue;
        }

        let trimmed = line.trim();
        if options.is_some() && trimmed == COPY_END_MARKER {
            let region = trim_blank_line_edges(region);
            if region.trim().is_empty() {
                return Err("quick-copy region is empty".to_string());
            }
            return Ok(Some(CopyRegion {
                body: region,
                options: options.unwrap_or_default(),
            }));
        }
        if options.is_none()
            && let Some(copy_options) = parse_copy_begin_marker(trimmed)?
        {
            options = Some(copy_options);
            region.clear();
            continue;
        }

        if let Some(fence) = MarkdownFence::opening(line) {
            if options.is_some() {
                region.push(line.to_string());
            }
            in_fence = Some(fence);
            continue;
        }

        if options.is_some() {
            region.push(line.to_string());
        }
    }

    if options.is_some() {
        Err("copy_begin marker without copy_end marker".to_string())
    } else {
        Ok(None)
    }
}

fn parse_copy_begin_marker(line: &str) -> Result<Option<CopyOptions>, String> {
    if !line.starts_with(COPY_BEGIN_PREFIX) {
        return Ok(None);
    }
    if !line.ends_with("-->") {
        return Err("invalid copy_begin marker".to_string());
    }

    let inner = line
        .strip_prefix("<!--")
        .and_then(|value| value.strip_suffix("-->"))
        .map(str::trim)
        .unwrap_or("");
    let mut parts = inner.split_whitespace();
    if parts.next() != Some("copy_begin") {
        return Ok(None);
    }

    let mut options = CopyOptions::default();
    for part in parts {
        if let Some(value) = part.strip_prefix("l=") {
            if options.lines.is_some() {
                return Err("duplicate copy_begin l attribute".to_string());
            }
            let lines = value
                .parse::<usize>()
                .map_err(|_| "copy_begin l must be a positive integer".to_string())?;
            if lines == 0 {
                return Err("copy_begin l must be a positive integer".to_string());
            }
            options.lines = Some(lines);
        } else if let Some(value) = part.strip_prefix("p=") {
            if options.prompt.is_some() {
                return Err("duplicate copy_begin p attribute".to_string());
            }
            let mut chars = value.chars();
            let prompt = chars
                .next()
                .ok_or_else(|| "copy_begin p must be $ or #".to_string())?;
            if chars.next().is_some() || !matches!(prompt, '$' | '#') {
                return Err("copy_begin p must be $ or #".to_string());
            }
            options.prompt = Some(prompt);
        } else {
            return Err(format!("unsupported copy_begin attribute: {part}"));
        }
    }

    Ok(Some(options))
}

impl CopyOptions {
    fn apply(&self, payload: &str) -> String {
        let mut lines = payload.lines().map(str::to_string).collect::<Vec<_>>();
        if let Some(limit) = self.lines {
            lines.truncate(limit);
        }
        if let Some(prompt) = self.prompt {
            lines = lines
                .into_iter()
                .map(|line| strip_shell_prompt(&line, prompt))
                .collect();
        }
        trim_blank_line_edges(lines)
    }
}

fn strip_shell_prompt(line: &str, prompt: char) -> String {
    let Some((prompt_index, ch)) = line.char_indices().find(|(_, ch)| !ch.is_whitespace()) else {
        return line.to_string();
    };
    if ch != prompt {
        return line.to_string();
    }

    let after_prompt = prompt_index + ch.len_utf8();
    if !line[after_prompt..].starts_with(' ') {
        return line.to_string();
    }

    format!("{}{}", &line[..prompt_index], &line[after_prompt + 1..])
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
#[path = "../tests/unit/quick_copy.rs"]
mod tests;
