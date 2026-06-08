use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::library::Library;

const TEMPLATE_FILE: &str = ".neith-note-template.md";

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
    let body = render_note_body(path, query);
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

fn render_note_body(path: &Path, query: &str) -> String {
    let slug = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("untitled");
    let title = title_from_slug(slug);
    let template = find_note_template(path).unwrap_or_else(default_note_template);
    template
        .replace("{{TITLE}}", &title)
        .replace("{{QUERY}}", query.trim())
        .replace("{{SLUG}}", slug)
        .replace("{{PATH}}", &path.display().to_string())
}

fn find_note_template(path: &Path) -> Option<String> {
    let mut current = path.parent();
    while let Some(dir) = current {
        let template = dir.join(TEMPLATE_FILE);
        if template.is_file()
            && let Ok(text) = fs::read_to_string(template)
        {
            return Some(text);
        }
        current = dir.parent();
    }
    None
}

fn default_note_template() -> String {
    "# {{TITLE}}\n\n> Manually created note. Verify commands and references before using them on important systems.\n\nTask: {{QUERY}}\n\n```bash\n# command or snippet\n```\n\n## References\n\n- TODO: add reference\n".to_string()
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

    #[test]
    fn creates_note_from_nearest_template() {
        let temp = tempfile::tempdir().unwrap();
        let category = temp.path().join("awk");
        fs::create_dir(&category).unwrap();
        fs::write(
            temp.path().join(TEMPLATE_FILE),
            "# {{TITLE}}\n\nTask: {{QUERY}}\nSlug: {{SLUG}}\n",
        )
        .unwrap();
        let path = category.join("print-selected-fields.md");

        create_note(&path, "awk print 3rd column").unwrap();

        assert_eq!(
            fs::read_to_string(path).unwrap(),
            "# Print Selected Fields\n\nTask: awk print 3rd column\nSlug: print-selected-fields\n"
        );
    }
}
