use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceFilter {
    All,
    Names,
    Content,
    Man,
}

impl SourceFilter {
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Names,
            Self::Names => Self::Content,
            Self::Content => Self::Man,
            Self::Man => Self::All,
        }
    }

    pub fn label(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Names => Some("names"),
            Self::Content => Some("content"),
            Self::Man => Some("man"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MatchMode {
    Fuzzy,
    Exact,
}

impl MatchMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Fuzzy => "fuzzy",
            Self::Exact => "exact",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LibraryScope {
    All,
    Alias(String),
}

impl LibraryScope {
    pub fn label(&self) -> &str {
        match self {
            Self::All => "all",
            Self::Alias(alias) => alias.as_str(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SearchRequest {
    pub query: String,
    pub filter: SourceFilter,
    pub mode: MatchMode,
    pub library: LibraryScope,
    pub limit: usize,
}

pub fn normalize_query(query: &str) -> String {
    query
        .split_whitespace()
        .map(normalize_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_token(token: &str) -> String {
    let lower = token.to_ascii_lowercase();
    if lower == "nth" || is_numeric_ordinal(&lower) || is_awk_field_ref(&lower) {
        "selected".to_string()
    } else {
        token.to_string()
    }
}

fn is_numeric_ordinal(token: &str) -> bool {
    let Some(number) = token
        .strip_suffix("st")
        .or_else(|| token.strip_suffix("nd"))
        .or_else(|| token.strip_suffix("rd"))
        .or_else(|| token.strip_suffix("th"))
    else {
        return false;
    };
    !number.is_empty() && number.chars().all(|ch| ch.is_ascii_digit())
}

fn is_awk_field_ref(token: &str) -> bool {
    token
        .strip_prefix('$')
        .is_some_and(|number| !number.is_empty() && number.chars().all(|ch| ch.is_ascii_digit()))
}

pub fn has_regex_meta(query: &str) -> bool {
    query.chars().any(|ch| {
        matches!(
            ch,
            '.' | '*' | '+' | '?' | '[' | ']' | '(' | ')' | '{' | '}' | '|'
        )
    })
}

pub fn longest_literal_seed(pattern: &str) -> Option<String> {
    pattern
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .filter(|part| part.len() >= 2)
        .max_by_key(|part| part.len())
        .map(str::to_string)
}

#[cfg(test)]
#[path = "../tests/unit/query.rs"]
mod tests;
