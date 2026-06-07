use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::search::SearchResult;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManQuery {
    pub title: String,
    pub section: Option<String>,
    pub remainder: String,
}

pub fn lookup_live_man(query: &str, cache_dir: &Path) -> Vec<SearchResult> {
    let Some(parts) = split_man_query(query) else {
        return Vec::new();
    };
    let mut results = Vec::new();
    for source in resolve_man_pages(&parts) {
        let Some(rendered) = render_man_page(cache_dir, &parts, &source) else {
            continue;
        };
        let label = man_label(&parts, &source);
        let body = fs::read_to_string(&rendered).unwrap_or_default();
        let search_text = if parts.remainder.is_empty() {
            parts.title.as_str()
        } else {
            parts.remainder.as_str()
        };
        let (line, snippet) = best_line(&body, search_text);
        results.push(SearchResult {
            title: format!("man: {label}"),
            path: rendered,
            rel_path: format!("man:{label}"),
            library_alias: "live-man".to_string(),
            source_kind: "man".to_string(),
            line,
            snippet,
            score: 20_000.0,
            rank_reason: "live-man".to_string(),
            body,
            is_live_man: true,
        });
    }
    results
}

pub fn split_man_query(query: &str) -> Option<ManQuery> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }
    let mut parts = query.split_whitespace();
    let first = parts.next()?;
    let rest = parts.collect::<Vec<_>>().join(" ");

    if let Some(open) = first.find('(')
        && first.ends_with(')')
        && open > 0
    {
        let title = first[..open].to_string();
        let section = first[open + 1..first.len() - 1].to_string();
        if is_man_section(&section) {
            return Some(ManQuery {
                title,
                section: Some(section),
                remainder: rest,
            });
        }
    }

    let mut parts = query.split_whitespace();
    let first = parts.next()?;
    if is_man_section(first) {
        let title = parts.next()?.to_string();
        let remainder = parts.collect::<Vec<_>>().join(" ");
        return Some(ManQuery {
            title,
            section: Some(first.to_string()),
            remainder,
        });
    }

    Some(ManQuery {
        title: first.to_string(),
        section: None,
        remainder: rest,
    })
}

fn resolve_man_pages(parts: &ManQuery) -> Vec<String> {
    let mut command = Command::new("man");
    command.arg("-wa").arg("--");
    if let Some(section) = &parts.section {
        command.arg(section);
    }
    command.arg(&parts.title);
    let Ok(output) = command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn render_man_page(cache_dir: &Path, parts: &ManQuery, source: &str) -> Option<PathBuf> {
    fs::create_dir_all(cache_dir).ok()?;
    let section = parts
        .section
        .clone()
        .or_else(|| section_from_path(source))
        .unwrap_or_else(|| "unknown".to_string());
    let file = cache_dir.join(format!(
        "{}.{}.{}.txt",
        safe_component(&parts.title),
        safe_component(&section),
        hash_text(source)
    ));
    if file.is_file() {
        return Some(file);
    }
    let tmp = file.with_extension("tmp");
    let mut man = Command::new("man")
        .arg("-l")
        .arg("--")
        .arg(source)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdout = man.stdout.take()?;
    let output = Command::new("col")
        .arg("-b")
        .stdin(Stdio::from(stdout))
        .output()
        .ok()?;
    let _ = man.wait();
    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }
    fs::write(&tmp, output.stdout).ok()?;
    fs::rename(&tmp, &file).ok()?;
    Some(file)
}

fn man_label(parts: &ManQuery, source: &str) -> String {
    let section = parts
        .section
        .clone()
        .or_else(|| section_from_path(source))
        .unwrap_or_else(|| "?".to_string());
    format!("{}({})", parts.title, section)
}

fn section_from_path(path: &str) -> Option<String> {
    let name = Path::new(path).file_name()?.to_string_lossy();
    let mut parts = name.split('.');
    parts.next()?;
    parts.next().map(str::to_string)
}

fn is_man_section(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_digit() || ch.is_ascii_alphabetic())
        && value.len() <= 4
}

fn safe_component(value: &str) -> String {
    let safe = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if safe.is_empty() {
        "man".to_string()
    } else {
        safe
    }
}

fn hash_text(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn best_line(body: &str, query: &str) -> (usize, String) {
    let terms = query
        .to_lowercase()
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    for (idx, line) in body.lines().enumerate() {
        let lower = line.to_lowercase();
        if !terms.is_empty() && terms.iter().any(|term| lower.contains(term)) {
            return (idx + 1, line.trim().to_string());
        }
    }
    body.lines()
        .enumerate()
        .find(|(_, line)| !line.trim().is_empty())
        .map(|(idx, line)| (idx + 1, line.trim().to_string()))
        .unwrap_or((1, String::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_name_section_query() {
        assert_eq!(
            split_man_query("printf(3) format"),
            Some(ManQuery {
                title: "printf".to_string(),
                section: Some("3".to_string()),
                remainder: "format".to_string()
            })
        );
    }

    #[test]
    fn parses_section_name_query() {
        assert_eq!(
            split_man_query("3 printf format").unwrap().section,
            Some("3".to_string())
        );
    }
}
