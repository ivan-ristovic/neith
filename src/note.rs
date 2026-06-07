use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::library::Library;

pub fn infer_note_path(libraries: &[Library], query: &str) -> Result<PathBuf> {
    let query = query.trim();
    let first = query.split_whitespace().next().unwrap_or("notes");
    let slug_source = if query.is_empty() { "untitled" } else { query };
    let slug = slugify(slug_source);

    let root = libraries
        .iter()
        .find(|library| library.alias == "neith-lib")
        .or_else(|| libraries.first())
        .context("no library available for note creation")?;

    let category = find_category(&root.path, first)
        .unwrap_or_else(|| PathBuf::from(sanitize_path_component(first)));
    Ok(root.path.join(category).join(format!("{slug}.md")))
}

pub fn create_note(path: &Path, query: &str) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let title = title_from_slug(
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("untitled"),
    );
    let body = format!(
        "# {title}\n\n> Manually created note. Verify commands and references before using them on important systems.\n\nTask: {query}\n\n```bash\n# command or snippet\n```\n\n## References\n\n- TODO: add reference\n"
    );
    fs::write(path, body)?;
    Ok(())
}

pub fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "untitled".to_string()
    } else {
        slug
    }
}

fn find_category(root: &Path, name: &str) -> Option<PathBuf> {
    let safe = sanitize_path_component(name);
    let candidate = root.join(&safe);
    candidate.is_dir().then_some(PathBuf::from(safe))
}

fn sanitize_path_component(value: &str) -> String {
    slugify(value)
}

fn title_from_slug(slug: &str) -> String {
    slug.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugifies_query() {
        assert_eq!(slugify("Awk print 3rd column"), "awk-print-3rd-column");
    }
}
