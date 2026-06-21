use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::Result;
use serde::Serialize;

use crate::action::command_exists;
use crate::config::RuntimeConfig;
use crate::indexer::{IndexStats, LibraryStatus, library_status};
use crate::library::Library;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckLevel {
    Ok,
    Warning,
    Failure,
}

#[derive(Clone, Debug, Serialize)]
pub struct Check {
    pub level: CheckLevel,
    pub name: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct HealthReport {
    pub checks: Vec<Check>,
}

impl HealthReport {
    pub fn exit_code(&self) -> i32 {
        if self
            .checks
            .iter()
            .any(|check| check.level == CheckLevel::Failure)
        {
            1
        } else if self
            .checks
            .iter()
            .any(|check| check.level == CheckLevel::Warning)
        {
            2
        } else {
            0
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct StatusRow {
    pub alias: String,
    pub kind: String,
    pub files: usize,
    pub indexed: usize,
    pub stale: usize,
    pub cache: String,
    pub index: String,
}

#[derive(Clone, Copy)]
enum CellStyle {
    None,
    Header,
    Alias,
    Dim,
    Green,
    Yellow,
    Red,
    Blue,
    Magenta,
}

struct Cell {
    text: String,
    style: CellStyle,
}

impl Cell {
    fn new(text: impl Into<String>, style: CellStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

pub fn collect_status_rows(libraries: &[Library]) -> Result<Vec<StatusRow>> {
    libraries
        .iter()
        .map(|library| library_status(library).map(status_row))
        .collect()
}

fn status_row(status: LibraryStatus) -> StatusRow {
    StatusRow {
        alias: status.alias,
        kind: status.kind,
        files: status.files,
        indexed: status.indexed,
        stale: status.stale,
        cache: status
            .cache_bytes
            .map(format_bytes)
            .unwrap_or_else(|| "missing".to_string()),
        index: status.index,
    }
}

pub fn healthcheck(config_path: Option<PathBuf>, libs_arg: Option<&str>) -> HealthReport {
    let mut checks = Vec::new();
    match RuntimeConfig::load(config_path, libs_arg) {
        Ok(runtime) => {
            checks.push(ok(
                "config",
                runtime
                    .path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "using defaults/env".to_string()),
            ));
            checks.push(ok(
                "libraries",
                format!("{} configured", runtime.libraries.len()),
            ));
            checks.extend(library_checks(&runtime.libraries));
            checks.extend(cache_index_checks(&runtime.libraries));
            checks.extend(tool_checks(&runtime));
        }
        Err(err) => {
            checks.push(failure("config", format!("{err:#}")));
        }
    }
    HealthReport { checks }
}

fn library_checks(libraries: &[Library]) -> Vec<Check> {
    let mut checks = Vec::new();
    let mut aliases = HashMap::<&str, usize>::new();
    for library in libraries {
        *aliases.entry(&library.alias).or_default() += 1;
        match std::fs::read_dir(&library.path) {
            Ok(_) => checks.push(ok(
                format!("library {}", library.alias),
                library.path.display().to_string(),
            )),
            Err(err) => checks.push(failure(
                format!("library {}", library.alias),
                format!("{}: {err}", library.path.display()),
            )),
        }
    }
    for (alias, count) in aliases {
        if count > 1 {
            checks.push(warning(
                format!("alias {alias}"),
                format!("used by {count} libraries"),
            ));
        }
    }
    checks
}

fn cache_index_checks(libraries: &[Library]) -> Vec<Check> {
    libraries
        .iter()
        .map(|library| match library_status(library) {
            Ok(status) if status.index == "ok" => ok(
                format!("index {}", library.alias),
                format!("{} files, cache {}", status.files, cache_label(&status)),
            ),
            Ok(status) => warning(
                format!("index {}", library.alias),
                format!(
                    "{} files, {} stale, cache {}, index {}",
                    status.files,
                    status.stale,
                    cache_label(&status),
                    status.index
                ),
            ),
            Err(err) => failure(format!("index {}", library.alias), format!("{err:#}")),
        })
        .collect()
}

fn cache_label(status: &LibraryStatus) -> String {
    status
        .cache_bytes
        .map(format_bytes)
        .unwrap_or_else(|| "missing".to_string())
}

fn tool_checks(runtime: &RuntimeConfig) -> Vec<Check> {
    let mut checks = Vec::new();
    let editor = runtime
        .app
        .editor
        .command
        .split_whitespace()
        .next()
        .unwrap_or("vi");
    if command_exists(editor) {
        checks.push(ok("editor", editor.to_string()));
    } else {
        checks.push(failure("editor", format!("{editor} not found in PATH")));
    }

    let clipboard_command = runtime.app.clipboard.command.trim();
    if let Some(command) = clipboard_command.split_whitespace().next() {
        if command_exists(command) {
            checks.push(ok(
                "clipboard",
                format!("custom command: {clipboard_command}"),
            ));
        } else {
            checks.push(warning(
                "clipboard",
                format!("custom command not found in PATH: {command}"),
            ));
        }
    } else if command_exists("wl-copy")
        || command_exists("xclip")
        || command_exists("xsel")
        || (std::env::var_os("TMUX").is_some() && command_exists("tmux"))
    {
        checks.push(ok("clipboard", "backend available"));
    } else {
        checks.push(warning(
            "clipboard",
            "no supported backend found (wl-copy, xclip, xsel, or tmux)",
        ));
    }

    for command in ["man", "col"] {
        if command_exists(command) {
            checks.push(ok(command, "available"));
        } else {
            checks.push(warning(command, "not found in PATH"));
        }
    }
    checks
}

pub fn render_healthcheck(report: &HealthReport, color: bool) -> String {
    let rows = report
        .checks
        .iter()
        .map(|check| {
            vec![
                Cell::new(
                    check_level_label(check.level),
                    check_level_style(check.level),
                ),
                Cell::new(check.name.clone(), CellStyle::Alias),
                Cell::new(check.detail.clone(), CellStyle::None),
            ]
        })
        .collect::<Vec<_>>();
    render_table(&["status", "check", "detail"], rows, color)
}

pub fn render_status_table(rows: &[StatusRow], color: bool) -> String {
    let rows = rows
        .iter()
        .map(|row| {
            vec![
                Cell::new(row.alias.clone(), CellStyle::Alias),
                Cell::new(row.kind.clone(), kind_style(&row.kind)),
                Cell::new(row.files.to_string(), CellStyle::None),
                Cell::new(row.indexed.to_string(), CellStyle::None),
                Cell::new(row.stale.to_string(), stale_style(row.stale)),
                Cell::new(row.cache.clone(), cache_style(&row.cache)),
                Cell::new(row.index.clone(), index_style(&row.index)),
            ]
        })
        .collect::<Vec<_>>();
    render_table(
        &[
            "alias", "kind", "files", "indexed", "stale", "cache", "index",
        ],
        rows,
        color,
    )
}

pub fn render_index_table<'a, I>(rows: I, color: bool) -> String
where
    I: IntoIterator<Item = (&'a Library, &'a IndexStats)>,
{
    let rows = rows
        .into_iter()
        .map(|(library, stats)| {
            vec![
                Cell::new(library.alias.clone(), CellStyle::Alias),
                Cell::new(
                    stats.indexed.to_string(),
                    count_style(stats.indexed, CellStyle::Green),
                ),
                Cell::new(
                    stats.removed.to_string(),
                    count_style(stats.removed, CellStyle::Yellow),
                ),
                Cell::new(stats.unchanged.to_string(), CellStyle::Dim),
            ]
        })
        .collect::<Vec<_>>();
    render_table(&["alias", "indexed", "removed", "unchanged"], rows, color)
}

fn render_table(headers: &[&str], rows: Vec<Vec<Cell>>, color: bool) -> String {
    let mut widths = headers
        .iter()
        .map(|header| header.len())
        .collect::<Vec<_>>();
    for row in &rows {
        for (idx, cell) in row.iter().enumerate() {
            if let Some(width) = widths.get_mut(idx) {
                *width = (*width).max(cell.text.len());
            }
        }
    }

    let mut lines = Vec::new();
    lines.push(render_cells(
        headers
            .iter()
            .map(|header| Cell::new(*header, CellStyle::Header))
            .collect(),
        &widths,
        color,
    ));
    for row in rows {
        lines.push(render_cells(row, &widths, color));
    }
    lines.join("\n")
}

fn render_cells(cells: Vec<Cell>, widths: &[usize], color: bool) -> String {
    cells
        .into_iter()
        .enumerate()
        .map(|(idx, cell)| {
            let padded = format!("{:<width$}", cell.text, width = widths[idx]);
            paint(&padded, cell.style, color)
        })
        .collect::<Vec<_>>()
        .join("  ")
        .trim_end()
        .to_string()
}

fn paint(text: &str, style: CellStyle, color: bool) -> String {
    if !color {
        return text.to_string();
    }
    let code = match style {
        CellStyle::None => return text.to_string(),
        CellStyle::Header => "1;2",
        CellStyle::Alias => "36",
        CellStyle::Dim => "2",
        CellStyle::Green => "32",
        CellStyle::Yellow => "33",
        CellStyle::Red => "31",
        CellStyle::Blue => "34",
        CellStyle::Magenta => "35",
    };
    format!("\x1b[{code}m{text}\x1b[0m")
}

fn check_level_label(level: CheckLevel) -> &'static str {
    match level {
        CheckLevel::Ok => "ok",
        CheckLevel::Warning => "warn",
        CheckLevel::Failure => "fail",
    }
}

fn check_level_style(level: CheckLevel) -> CellStyle {
    match level {
        CheckLevel::Ok => CellStyle::Green,
        CheckLevel::Warning => CellStyle::Yellow,
        CheckLevel::Failure => CellStyle::Red,
    }
}

fn kind_style(kind: &str) -> CellStyle {
    match kind {
        "note" => CellStyle::Green,
        "devdocs" => CellStyle::Blue,
        "man" => CellStyle::Magenta,
        "mixed" => CellStyle::Yellow,
        _ => CellStyle::Dim,
    }
}

fn stale_style(stale: usize) -> CellStyle {
    match stale {
        0 => CellStyle::Green,
        1..=99 => CellStyle::Yellow,
        _ => CellStyle::Red,
    }
}

fn cache_style(cache: &str) -> CellStyle {
    if cache == "missing" {
        CellStyle::Red
    } else {
        CellStyle::Dim
    }
}

fn index_style(index: &str) -> CellStyle {
    match index {
        "ok" => CellStyle::Green,
        "stale" => CellStyle::Yellow,
        "missing" => CellStyle::Red,
        _ => CellStyle::Dim,
    }
}

fn count_style(count: usize, nonzero: CellStyle) -> CellStyle {
    if count == 0 { CellStyle::Dim } else { nonzero }
}

pub fn use_color_stdout() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

fn ok(name: impl Into<String>, detail: impl Into<String>) -> Check {
    Check {
        level: CheckLevel::Ok,
        name: name.into(),
        detail: detail.into(),
    }
}

fn warning(name: impl Into<String>, detail: impl Into<String>) -> Check {
    Check {
        level: CheckLevel::Warning,
        name: name.into(),
        detail: detail.into(),
    }
}

fn failure(name: impl Into<String>, detail: impl Into<String>) -> Check {
    Check {
        level: CheckLevel::Failure,
        name: name.into(),
        detail: detail.into(),
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes}B")
    } else if value >= 10.0 {
        format!("{value:.0}{}", UNITS[unit])
    } else {
        format!("{value:.1}{}", UNITS[unit])
    }
}

#[cfg(test)]
#[path = "../tests/unit/diagnostics.rs"]
mod tests;
