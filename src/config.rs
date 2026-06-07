use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::library::Library;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub libraries: Vec<LibraryConfig>,

    #[serde(default)]
    pub editor: EditorConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryConfig {
    pub path: PathBuf,
    pub alias: Option<String>,
    pub pinned: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EditorConfig {
    #[serde(default = "default_editor_command")]
    pub command: String,

    #[serde(default)]
    pub return_behavior: EditorReturn,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum EditorReturn {
    #[default]
    Exit,
    Resume,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            command: default_editor_command(),
            return_behavior: EditorReturn::Exit,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub path: Option<PathBuf>,
    pub app: AppConfig,
    pub libraries: Vec<Library>,
}

impl RuntimeConfig {
    pub fn load(config_path: Option<PathBuf>, libs_arg: Option<&str>) -> Result<Self> {
        let path = config_path.or_else(default_config_path);
        let mut app = if let Some(path) = &path {
            if path.is_file() {
                let text = fs::read_to_string(path)
                    .with_context(|| format!("failed to read config {}", path.display()))?;
                toml::from_str(&text)
                    .with_context(|| format!("failed to parse config {}", path.display()))?
            } else {
                AppConfig::default()
            }
        } else {
            AppConfig::default()
        };

        if app.editor.command.trim().is_empty() {
            app.editor.command = default_editor_command();
        }

        let mut libraries = Vec::new();
        for library in &app.libraries {
            libraries.push(Library::new(
                expand_tilde(&library.path),
                library.alias.clone(),
                library.pinned,
            ));
        }

        if let Ok(env_libs) = env::var("NEITH_LIBS") {
            extend_library_paths(&mut libraries, &env_libs);
        }

        if let Some(libs_arg) = libs_arg {
            extend_library_paths(&mut libraries, libs_arg);
        }

        dedupe_libraries(&mut libraries);
        libraries.retain(|library| library.path.is_dir());

        if libraries.is_empty() {
            bail!("no libraries configured; run `neith config init` or pass `--libs`");
        }

        Ok(Self {
            path,
            app,
            libraries,
        })
    }

    pub fn init_config(
        path: Option<PathBuf>,
        force: bool,
        libs_arg: Option<&str>,
    ) -> Result<PathBuf> {
        let path = path
            .or_else(default_config_path)
            .context("failed to resolve config path")?;
        if path.exists() && !force {
            bail!(
                "config already exists: {} (use --force to overwrite)",
                path.display()
            );
        }
        let runtime = Self::load(Some(path.clone()), libs_arg)?;
        let app = AppConfig {
            libraries: runtime
                .libraries
                .iter()
                .map(|library| LibraryConfig {
                    path: library.path.clone(),
                    alias: Some(library.alias.clone()),
                    pinned: Some(library.pinned),
                })
                .collect(),
            editor: runtime.app.editor,
        };
        let text = toml::to_string_pretty(&app)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, text)?;
        Ok(path)
    }
}

pub fn default_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("neith").join("config.toml"))
}

fn default_editor_command() -> String {
    env::var("EDITOR").unwrap_or_else(|_| "vi".to_string())
}

fn extend_library_paths(libraries: &mut Vec<Library>, colon_list: &str) {
    for raw in colon_list.split(':') {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        libraries.push(Library::new(expand_tilde(Path::new(raw)), None, None));
    }
}

fn dedupe_libraries(libraries: &mut Vec<Library>) {
    let mut seen = HashSet::new();
    libraries.retain(|library| {
        let key = library
            .path
            .canonicalize()
            .unwrap_or_else(|_| library.path.clone());
        seen.insert(key)
    });
}

fn expand_tilde(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_default_is_exit() {
        assert_eq!(EditorConfig::default().return_behavior, EditorReturn::Exit);
    }
}
