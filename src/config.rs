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

    #[serde(default)]
    pub clipboard: ClipboardConfig,

    #[serde(default)]
    pub ui: UiConfig,
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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ClipboardConfig {
    #[serde(default)]
    pub command: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_preview_cursor_percent")]
    pub preview_cursor_percent: u8,

    #[serde(default)]
    pub preview_syntax: PreviewSyntax,

    #[serde(default)]
    pub preview_bat_args: Vec<String>,

    #[serde(default)]
    pub prompt: PromptConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromptConfig {
    #[serde(default = "default_prompt_separator")]
    pub separator: String,

    #[serde(default = "default_prompt_right_separator")]
    pub right_separator: String,

    #[serde(default)]
    pub colors: PromptColorsConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromptColorsConfig {
    #[serde(default = "default_prompt_fuzzy_color")]
    pub fuzzy: String,

    #[serde(default = "default_prompt_exact_color")]
    pub exact: String,

    #[serde(default = "default_prompt_scope_color")]
    pub scope: String,

    #[serde(default = "default_prompt_filter_color")]
    pub filter: String,

    #[serde(default = "default_prompt_separator_color")]
    pub separator: String,

    #[serde(default = "default_prompt_marker_color")]
    pub marker: String,

    #[serde(default = "default_prompt_query_color")]
    pub query: String,

    #[serde(default = "default_prompt_add_color")]
    pub add: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum EditorReturn {
    #[default]
    Exit,
    Resume,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PreviewSyntax {
    #[default]
    Auto,
    Plain,
    Bat,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            command: default_editor_command(),
            return_behavior: EditorReturn::Exit,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            preview_cursor_percent: default_preview_cursor_percent(),
            preview_syntax: PreviewSyntax::Auto,
            preview_bat_args: Vec::new(),
            prompt: PromptConfig::default(),
        }
    }
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            separator: default_prompt_separator(),
            right_separator: default_prompt_right_separator(),
            colors: PromptColorsConfig::default(),
        }
    }
}

impl Default for PromptColorsConfig {
    fn default() -> Self {
        Self {
            fuzzy: default_prompt_fuzzy_color(),
            exact: default_prompt_exact_color(),
            scope: default_prompt_scope_color(),
            filter: default_prompt_filter_color(),
            separator: default_prompt_separator_color(),
            marker: default_prompt_marker_color(),
            query: default_prompt_query_color(),
            add: default_prompt_add_color(),
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
        app.clipboard.command = app.clipboard.command.trim().to_string();

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
            clipboard: runtime.app.clipboard,
            ui: runtime.app.ui,
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

fn default_preview_cursor_percent() -> u8 {
    50
}

fn default_prompt_separator() -> String {
    ":".to_string()
}

fn default_prompt_right_separator() -> String {
    ">".to_string()
}

fn default_prompt_fuzzy_color() -> String {
    "cyan".to_string()
}

fn default_prompt_exact_color() -> String {
    "red".to_string()
}

fn default_prompt_scope_color() -> String {
    "blue".to_string()
}

fn default_prompt_filter_color() -> String {
    "green".to_string()
}

fn default_prompt_separator_color() -> String {
    "dark-gray".to_string()
}

fn default_prompt_marker_color() -> String {
    "dark-gray".to_string()
}

fn default_prompt_query_color() -> String {
    "white".to_string()
}

fn default_prompt_add_color() -> String {
    "cyan".to_string()
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

    #[test]
    fn ui_default_centers_preview_cursor() {
        assert_eq!(UiConfig::default().preview_cursor_percent, 50);
    }

    #[test]
    fn clipboard_default_command_is_empty() {
        assert_eq!(ClipboardConfig::default().command, "");
    }

    #[test]
    fn parses_clipboard_command() {
        let app: AppConfig =
            toml::from_str("[clipboard]\ncommand = \"xclip -sel clip\"\n").unwrap();

        assert_eq!(app.clipboard.command, "xclip -sel clip");
    }

    #[test]
    fn ui_default_uses_auto_preview_syntax() {
        assert_eq!(UiConfig::default().preview_syntax, PreviewSyntax::Auto);
        assert!(UiConfig::default().preview_bat_args.is_empty());
    }

    #[test]
    fn ui_default_prompt_uses_separator_and_colors() {
        let prompt = UiConfig::default().prompt;

        assert_eq!(prompt.separator, ":");
        assert_eq!(prompt.right_separator, ">");
        assert_eq!(prompt.colors.fuzzy, "cyan");
        assert_eq!(prompt.colors.exact, "red");
        assert_eq!(prompt.colors.scope, "blue");
        assert_eq!(prompt.colors.filter, "green");
        assert_eq!(prompt.colors.separator, "dark-gray");
        assert_eq!(prompt.colors.marker, "dark-gray");
        assert_eq!(prompt.colors.query, "white");
        assert_eq!(prompt.colors.add, "cyan");
    }

    #[test]
    fn parses_ui_preview_cursor_percent() {
        let app: AppConfig = toml::from_str("[ui]\npreview_cursor_percent = 35\n").unwrap();

        assert_eq!(app.ui.preview_cursor_percent, 35);
    }

    #[test]
    fn parses_ui_preview_syntax_and_bat_args() {
        let app: AppConfig = toml::from_str(
            "[ui]\npreview_syntax = \"plain\"\npreview_bat_args = [\"--theme=TwoDark\"]\n",
        )
        .unwrap();

        assert_eq!(app.ui.preview_syntax, PreviewSyntax::Plain);
        assert_eq!(app.ui.preview_bat_args, vec!["--theme=TwoDark"]);
    }

    #[test]
    fn parses_ui_prompt_separator_and_colors() {
        let app: AppConfig = toml::from_str(
            "[ui.prompt]\nseparator = \"/\"\nright_separator = \"|\"\n\n[ui.prompt.colors]\nfuzzy = \"blue\"\nquery = \"white\"\n",
        )
        .unwrap();

        assert_eq!(app.ui.prompt.separator, "/");
        assert_eq!(app.ui.prompt.right_separator, "|");
        assert_eq!(app.ui.prompt.colors.fuzzy, "blue");
        assert_eq!(app.ui.prompt.colors.query, "white");
        assert_eq!(app.ui.prompt.colors.exact, "red");
    }

    #[test]
    fn sample_config_parses() {
        let sample = include_str!("../config-sample.toml");
        let app: AppConfig = toml::from_str(sample).unwrap();

        assert_eq!(app.ui.prompt.separator, ":");
        assert_eq!(app.ui.prompt.right_separator, ">");
        assert_eq!(app.ui.prompt.colors.query, "white");
    }
}
