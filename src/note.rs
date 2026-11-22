use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

use anyhow::{Context, Result, bail};
use tempfile::{Builder, NamedTempFile};
use walkdir::WalkDir;

use crate::library::Library;

const TEMPLATE_FILE: &str = ".neith-note-template.md";

#[derive(Debug)]
pub enum NoteEdit {
    Existing(PathBuf),
    Draft(NoteDraft),
}

#[derive(Debug)]
pub struct NoteDraft {
    target: PathBuf,
    file: NamedTempFile,
    initial_body: String,
    initial_modified: SystemTime,
}

#[derive(Debug, Eq, PartialEq)]
pub enum NoteEditOutcome {
    OpenedExisting,
    Created(PathBuf),
    Discarded,
}

impl NoteEdit {
    pub fn editor_path(&self) -> &Path {
        match self {
            Self::Existing(path) => path,
            Self::Draft(draft) => draft.file.path(),
        }
    }

    pub fn finish(self, editor_succeeded: bool) -> Result<NoteEditOutcome> {
        match self {
            Self::Existing(_) => Ok(NoteEditOutcome::OpenedExisting),
            Self::Draft(draft) => draft.finish(editor_succeeded),
        }
    }
}

impl NoteDraft {
    fn finish(self, editor_succeeded: bool) -> Result<NoteEditOutcome> {
        if !editor_succeeded {
            return match self.contents() {
                Ok((_, false)) => Ok(NoteEditOutcome::Discarded),
                Ok((_, true)) => Err(self.preserve(anyhow::anyhow!(
                    "editor exited unsuccessfully after saving the draft"
                ))),
                Err(err) => Err(self.preserve(err)),
            };
        }
        match self.save() {
            Ok(false) => Ok(NoteEditOutcome::Discarded),
            Ok(true) => Ok(NoteEditOutcome::Created(self.target)),
            Err(err) => Err(self.preserve(err)),
        }
    }

    fn save(&self) -> Result<bool> {
        let (body, changed) = self.contents()?;
        if !changed {
            return Ok(false);
        }
        if let Some(parent) = self.target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut target = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.target)
            .with_context(|| format!("failed to create {}", self.target.display()))?;
        if let Err(err) = target.write_all(&body) {
            drop(target);
            let _ = fs::remove_file(&self.target);
            return Err(err).with_context(|| format!("failed to write {}", self.target.display()));
        }
        Ok(true)
    }

    fn contents(&self) -> Result<(Vec<u8>, bool)> {
        let body = fs::read(self.file.path())?;
        let modified = fs::metadata(self.file.path())?.modified()?;
        let changed = body != self.initial_body.as_bytes() || modified != self.initial_modified;
        Ok((body, changed))
    }

    fn preserve(self, err: anyhow::Error) -> anyhow::Error {
        match self.file.keep() {
            Ok((_, path)) => anyhow::anyhow!("{err:#}; edited draft kept at {}", path.display()),
            Err(keep_err) => {
                anyhow::anyhow!("{err:#}; could not preserve edited draft: {keep_err}")
            }
        }
    }
}

pub fn prepare_note_edit(path: &Path, query: &str) -> Result<NoteEdit> {
    validate_note_title(query)?;
    if path.is_file() {
        return Ok(NoteEdit::Existing(path.to_path_buf()));
    }
    if path.exists() {
        bail!("{} exists and is not a regular file", path.display());
    }
    let body = render_note_body(path, query);
    let mut file = Builder::new()
        .prefix("neith-draft-")
        .suffix(".md")
        .tempfile()
        .context("failed to create note draft")?;
    file.write_all(body.as_bytes())?;
    let initial_modified = file.as_file().metadata()?.modified()?;
    Ok(NoteEdit::Draft(NoteDraft {
        target: path.to_path_buf(),
        file,
        initial_body: body,
        initial_modified,
    }))
}

pub fn validate_note_title(title: &str) -> Result<()> {
    if !title.chars().any(char::is_alphanumeric) {
        bail!("title needs a letter or number");
    }
    Ok(())
}

pub fn infer_note_path(libraries: &[Library], query: &str) -> Result<PathBuf> {
    infer_note_path_in_library(target_library(libraries)?, query)
}

pub fn target_library(libraries: &[Library]) -> Result<&Library> {
    libraries
        .iter()
        .find(|library| library.alias == "neith-lib")
        .or_else(|| libraries.first())
        .context("no library available for note creation")
}

pub fn infer_note_path_from_args(libraries: &[Library], args: &[String]) -> Result<PathBuf> {
    let Some(folder) = args.first() else {
        bail!("provide a folder and title: neith add <folder> <title>");
    };
    let title = args[1..].join(" ");
    infer_note_path_from_parts(target_library(libraries)?, folder, &title)
}

pub fn infer_note_path_in_library(root: &Library, query: &str) -> Result<PathBuf> {
    let (folder, title) = parse_note_input(query)?;
    infer_note_path_from_parts(root, &folder, &title)
}

pub fn infer_note_path_from_parts(root: &Library, folder: &str, title: &str) -> Result<PathBuf> {
    validate_note_title(title)?;
    let folder = folder.trim();
    let relative = Path::new(folder);
    if folder.is_empty()
        || folder.chars().any(char::is_control)
        || !relative
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        bail!("folder must be a relative path inside the library");
    }
    let mut category = PathBuf::new();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            unreachable!()
        };
        let exact = root.path.join(&category).join(name);
        if exact.is_dir() {
            category.push(name);
            continue;
        }
        if exact.exists() {
            bail!("{} is not a directory", exact.display());
        }
        let name = name.to_string_lossy();
        if !name.chars().any(char::is_alphanumeric) {
            bail!("folder needs a letter or number in each path component");
        }
        let safe = slugify(&name);
        let safe_path = root.path.join(&category).join(&safe);
        if safe_path.exists() && !safe_path.is_dir() {
            bail!("{} is not a directory", safe_path.display());
        }
        category.push(safe);
    }
    let folder_path = root.path.join(&category);
    let root_real = fs::canonicalize(&root.path)?;
    let mut existing = folder_path.as_path();
    while !existing.exists() {
        existing = existing.parent().context("folder has no existing parent")?;
    }
    if !fs::canonicalize(existing)?.starts_with(&root_real) {
        bail!("folder resolves outside the library");
    }
    Ok(folder_path.join(format!("{}.md", slugify(title))))
}

pub fn format_note_input(folder: &str, title: &str) -> String {
    let token = if folder.chars().any(char::is_whitespace) || folder.contains(['"', '\\']) {
        format!("\"{}\"", folder.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        folder.to_string()
    };
    format!("{token} {title}")
}

pub fn replace_note_folder(input: &str, folder: &str) -> String {
    let title = split_folder_prefix(input)
        .map(|(_, title)| title)
        .unwrap_or_default();
    format_note_input(folder, &title)
}

fn parse_note_input(input: &str) -> Result<(String, String)> {
    let (folder, title) = split_folder_prefix(input)?;
    if folder.is_empty() {
        bail!("provide a folder and title: <folder> <title>");
    }
    validate_note_title(&title)?;
    Ok((folder, title))
}

pub fn validate_note_input(input: &str) -> Result<()> {
    parse_note_input(input).map(|_| ())
}

fn split_folder_prefix(input: &str) -> Result<(String, String)> {
    let input = input.trim_start();
    if let Some(rest) = input.strip_prefix('"') {
        let mut folder = String::new();
        let mut escaped = false;
        for (offset, ch) in rest.char_indices() {
            if escaped {
                folder.push(ch);
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                return Ok((folder, rest[offset + 1..].trim().to_string()));
            } else {
                folder.push(ch);
            }
        }
        bail!("close the quoted folder name");
    }
    let split = input.find(char::is_whitespace).unwrap_or(input.len());
    Ok((
        input[..split].to_string(),
        input[split..].trim().to_string(),
    ))
}

pub fn list_note_folders(root: &Library) -> Result<Vec<String>> {
    let mut folders = Vec::new();
    for entry in WalkDir::new(&root.path)
        .follow_links(false)
        .min_depth(1)
        .into_iter()
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !entry.file_type().is_dir() || (!name.starts_with('.') && name != "target")
        })
    {
        let entry = entry?;
        if !entry.file_type().is_dir() {
            continue;
        }
        let relative = entry.path().strip_prefix(&root.path)?;
        if let Some(value) = relative.to_str()
            && !value.chars().any(char::is_control)
        {
            folders.push(value.to_string());
        }
    }
    folders.sort();
    Ok(folders)
}

pub fn pick_note_folder(root: &Library) -> Result<Option<String>> {
    pick_note_folder_with_program(root, "fzf")
}

fn pick_note_folder_with_program(root: &Library, program: &str) -> Result<Option<String>> {
    let folders = list_note_folders(root)?;
    if folders.is_empty() {
        bail!("no folders found in {}", root.path.display());
    }
    let mut child = Command::new(program)
        .args(["--prompt", "Folder> ", "--height", "40%", "--reverse"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("failed to start {program}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        for folder in &folders {
            if let Err(err) = writeln!(stdin, "{folder}") {
                if err.kind() == ErrorKind::BrokenPipe {
                    break;
                }
                return Err(err.into());
            }
        }
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return if matches!(output.status.code(), Some(1 | 130)) {
            Ok(None)
        } else {
            bail!("fzf exited with {}", output.status)
        };
    }
    let selected = String::from_utf8(output.stdout)?
        .trim_end_matches(['\n', '\r'])
        .to_string();
    if folders.binary_search(&selected).is_err() {
        bail!("fzf selected an unknown folder");
    }
    Ok(Some(selected))
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
        if ch.is_alphanumeric() {
            slug.extend(ch.to_lowercase());
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
    "# {{TITLE}}\n\n> Manually created note. Verify commands and references before using them on important systems.\n\nTask: {{QUERY}}\n\n<!-- copy_begin -->\n```bash\n# command or snippet\n```\n<!-- copy_end -->\n\n## References\n\n- TODO: add reference\n".to_string()
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
#[path = "../tests/unit/note.rs"]
mod tests;
