use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use walkdir::{DirEntry, WalkDir};

const NEITHIGNORE_FILE: &str = ".neithignore";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Library {
    pub alias: String,
    pub path: PathBuf,
    pub pinned: bool,
}

impl Library {
    pub fn new(path: PathBuf, alias: Option<String>, pinned: Option<bool>) -> Self {
        let alias = alias.unwrap_or_else(|| infer_alias(&path));
        let pinned = pinned.unwrap_or_else(|| is_default_pinned_alias(&alias));
        Self {
            alias,
            path,
            pinned,
        }
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.path.join(".neith-cache").join("neith")
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceKind {
    Note,
    Devdocs,
    Man,
}

impl SourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Devdocs => "devdocs",
            Self::Man => "man",
        }
    }
}

impl std::str::FromStr for SourceKind {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "man" => Ok(Self::Man),
            "devdocs" => Ok(Self::Devdocs),
            _ => Ok(Self::Note),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntryDoc {
    pub library_alias: String,
    pub library_path: PathBuf,
    pub path: PathBuf,
    pub rel_path: String,
    pub title: String,
    pub body: String,
    pub excerpt: String,
    pub source_kind: SourceKind,
    pub size: u64,
    pub modified_unix: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileSignature {
    pub path: String,
    pub rel_path: String,
    pub title: String,
    pub excerpt: String,
    pub source_kind: SourceKind,
    pub size: u64,
    pub modified_unix: u64,
    pub content_hash: u64,
}

pub fn discover_markdown_entries(library: &Library) -> Result<Vec<EntryDoc>> {
    let mut entries = Vec::new();
    let ignore = LibraryIgnore::load(&library.path)?;
    for item in WalkDir::new(&library.path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| should_descend(entry) && !ignore.matches(&library.path, entry.path()))
    {
        let item = item?;
        if !item.file_type().is_file()
            || item.path().extension().and_then(|ext| ext.to_str()) != Some("md")
            || ignore.matches(&library.path, item.path())
        {
            continue;
        }
        entries.push(read_entry(library, item.path())?);
    }
    entries.sort_by(|left, right| left.rel_path.cmp(&right.rel_path));
    Ok(entries)
}

pub fn discover_signatures(library: &Library) -> Result<Vec<FileSignature>> {
    discover_markdown_entries(library).map(|entries| {
        entries
            .into_iter()
            .map(|entry| FileSignature {
                content_hash: content_hash(&entry.body),
                path: entry.path.to_string_lossy().to_string(),
                rel_path: entry.rel_path,
                title: entry.title,
                excerpt: entry.excerpt,
                source_kind: entry.source_kind,
                size: entry.size,
                modified_unix: entry.modified_unix,
            })
            .collect()
    })
}

pub fn read_entry(library: &Library, path: &Path) -> Result<EntryDoc> {
    let body = fs::read_to_string(path)
        .with_context(|| format!("failed to read markdown entry {}", path.display()))?;
    let metadata = fs::metadata(path)?;
    let modified_unix = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let rel_path = path
        .strip_prefix(&library.path)
        .unwrap_or(path)
        .to_string_lossy()
        .trim_start_matches('/')
        .to_string();
    let title = extract_title(&body).unwrap_or_else(|| title_from_path(path));
    let source_kind = classify_source(&library.path, &rel_path, &body);
    let excerpt = build_excerpt(&body, 900);

    Ok(EntryDoc {
        library_alias: library.alias.clone(),
        library_path: library.path.clone(),
        path: path.to_path_buf(),
        rel_path,
        title,
        body,
        excerpt,
        source_kind,
        size: metadata.len(),
        modified_unix,
    })
}

pub fn extract_title(body: &str) -> Option<String> {
    body.lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|title| !title.is_empty())
        .map(str::to_string)
}

pub fn title_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("untitled")
        .replace('-', " ")
}

fn build_excerpt(body: &str, limit: usize) -> String {
    let text = body
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(12)
        .collect::<Vec<_>>()
        .join(" ");
    if text.len() <= limit {
        text
    } else {
        format!("{}...", truncate_to_char_boundary(&text, limit))
    }
}

pub fn content_hash(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn truncate_to_char_boundary(value: &str, limit: usize) -> &str {
    if value.len() <= limit {
        return value;
    }
    let mut end = limit;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

fn classify_source(library_path: &Path, rel_path: &str, body: &str) -> SourceKind {
    if body.starts_with("# man:") {
        return SourceKind::Man;
    }
    let path_text = library_path.to_string_lossy();
    if path_text.contains("neith-devdocs/generated")
        || body.contains("Generated from DevDocs")
        || body.contains("DevDocs path:")
    {
        if rel_path.starts_with("man/") {
            return SourceKind::Man;
        }
        return SourceKind::Devdocs;
    }
    SourceKind::Note
}

fn should_descend(entry: &DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    if !entry.file_type().is_dir() {
        return true;
    }
    !matches!(name.as_ref(), ".git" | ".neith-cache" | "target" | ".cache")
}

#[derive(Debug, Default)]
struct LibraryIgnore {
    exact_paths: Vec<String>,
    dir_prefixes: Vec<String>,
}

impl LibraryIgnore {
    fn load(root: &Path) -> Result<Self> {
        let path = root.join(NEITHIGNORE_FILE);
        if !path.is_file() {
            return Ok(Self::default());
        }

        let text = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let mut ignore = Self::default();
        for line in text.lines() {
            let rule = line.trim();
            if rule.is_empty() || rule.starts_with('#') {
                continue;
            }
            let rule = rule.trim_start_matches("./").to_string();
            if rule.ends_with('/') {
                ignore.dir_prefixes.push(rule);
            } else {
                ignore.exact_paths.push(rule);
            }
        }
        Ok(ignore)
    }

    fn matches(&self, root: &Path, path: &Path) -> bool {
        let rel_path = match path.strip_prefix(root) {
            Ok(value) => value,
            Err(_) => return false,
        };
        let rel_path = rel_path
            .to_string_lossy()
            .trim_start_matches('/')
            .to_string();
        if rel_path.is_empty() {
            return false;
        }
        if self.exact_paths.iter().any(|rule| rule == &rel_path) {
            return true;
        }
        self.dir_prefixes.iter().any(|prefix| {
            let dir = prefix.trim_end_matches('/');
            rel_path == dir || rel_path.starts_with(prefix)
        })
    }
}

pub fn infer_alias(path: &Path) -> String {
    let normalized = path.to_string_lossy();
    if normalized.contains("neith-devdocs/generated") {
        return "devdocs".to_string();
    }
    if normalized.ends_with("neith-lib") || normalized.ends_with("neith-lib/") {
        return "neith-lib".to_string();
    }
    if normalized.ends_with("ol-docs") || normalized.ends_with("ol-docs/") {
        return "ol-docs".to_string();
    }
    path.components()
        .rev()
        .find_map(|component| {
            let value = component.as_os_str().to_string_lossy();
            (!value.is_empty() && value != "docs").then(|| value.to_string())
        })
        .unwrap_or_else(|| "library".to_string())
}

fn is_default_pinned_alias(alias: &str) -> bool {
    matches!(alias, "neith-lib" | "devdocs" | "ol-docs")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_markdown_title() {
        assert_eq!(extract_title("# Hello\nbody"), Some("Hello".to_string()));
    }

    #[test]
    fn classifies_man_doc() {
        assert_eq!(
            classify_source(Path::new("/tmp/lib"), "man/awk.md", "# man: awk"),
            SourceKind::Man
        );
    }

    #[test]
    fn infers_known_aliases() {
        assert_eq!(
            infer_alias(Path::new("/home/ivan/neith/neith-devdocs/generated")),
            "devdocs"
        );
        assert_eq!(
            infer_alias(Path::new("/home/ivan/neith/neith-lib")),
            "neith-lib"
        );
    }

    #[test]
    fn library_ignore_excludes_exact_files_and_directory_prefixes() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join(".neithignore"),
            "AGENTS.md\n.hidden/\nskip/\n",
        )
        .unwrap();
        fs::write(temp.path().join("AGENTS.md"), "# Instructions\n").unwrap();
        fs::write(temp.path().join("keep.md"), "# Keep\n").unwrap();
        fs::create_dir(temp.path().join("skip")).unwrap();
        fs::write(temp.path().join("skip").join("note.md"), "# Skip\n").unwrap();
        fs::create_dir(temp.path().join(".hidden")).unwrap();
        fs::write(temp.path().join(".hidden").join("note.md"), "# Hidden\n").unwrap();

        let library = Library::new(temp.path().to_path_buf(), Some("test".to_string()), None);
        let rel_paths = discover_markdown_entries(&library)
            .unwrap()
            .into_iter()
            .map(|entry| entry.rel_path)
            .collect::<Vec<_>>();

        assert_eq!(rel_paths, vec!["keep.md"]);
    }

    #[test]
    fn library_ignore_comments_and_blank_lines_are_ignored() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join(".neithignore"),
            "\n# comment\n./ignored.md\n",
        )
        .unwrap();
        fs::write(temp.path().join("ignored.md"), "# Ignored\n").unwrap();
        fs::write(temp.path().join("included.md"), "# Included\n").unwrap();

        let library = Library::new(temp.path().to_path_buf(), Some("test".to_string()), None);
        let rel_paths = discover_markdown_entries(&library)
            .unwrap()
            .into_iter()
            .map(|entry| entry.rel_path)
            .collect::<Vec<_>>();

        assert_eq!(rel_paths, vec!["included.md"]);
    }

    #[test]
    fn excerpt_truncates_on_utf8_boundary() {
        let body = format!("# {}\n", "é".repeat(600));
        let excerpt = build_excerpt(&body, 900);
        assert!(excerpt.ends_with("..."));
        assert!(excerpt.is_char_boundary(excerpt.len()));
    }
}
