use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use ansi_to_tui::IntoText as _;
use anyhow::Result;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    MouseEvent, MouseEventKind,
};
use crossterm::execute;
use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config as FuzzyConfig, Matcher, Utf32Str};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};

use crate::action::{copy_text, open_editor, open_editor_in_tmux_pane};
use crate::config::{EditorReturn, PreviewSyntax, RuntimeConfig, UiConfig};
use crate::indexer::{IndexManager, ensure_indexes};
use crate::note;
use crate::query::{LibraryScope, MatchMode, SearchRequest, SourceFilter, normalize_query};
use crate::quick_copy::{self, CodeBlock, ExtractedCopy};
use crate::search::{SearchEngine, SearchResult};

const MOUSE_SCROLL_LINES: isize = 3;
const BAT_PREVIEW_ARGS: &[&str] = &[
    "--color=always",
    "--paging=never",
    "--style=plain",
    "--wrap=never",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Focus {
    Results,
    Preview,
    LibrarySelector,
    AddEntry,
    QuickCopy,
    Help,
}

#[derive(Default)]
struct PreviewState {
    path: Option<PathBuf>,
    lines: Vec<String>,
    syntax_lines: Vec<Line<'static>>,
    scroll: usize,
    cursor: usize,
    viewport_height: usize,
    copy_mode: bool,
    anchor: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EditorRequest {
    path: PathBuf,
    line: Option<usize>,
}

#[derive(Clone, Debug)]
struct ResultRefineState {
    base_query: String,
    base_query_cursor: usize,
    base_results: Vec<SearchResult>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AddEntryState {
    note_query: String,
    path_text: String,
    path_cursor: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct QuickCopyState {
    blocks: Vec<CodeBlock>,
    selected: usize,
    return_focus: Focus,
}

pub struct App {
    config: RuntimeConfig,
    engine: SearchEngine,
    query: String,
    query_cursor: usize,
    filter: SourceFilter,
    mode: MatchMode,
    focus: Focus,
    previous_focus: Focus,
    results: Vec<SearchResult>,
    selected: usize,
    preview: PreviewState,
    preview_area: Option<Rect>,
    status: String,
    library_scopes: Vec<LibraryScope>,
    library_index: usize,
    library_selector_index: usize,
    result_refine: Option<ResultRefineState>,
    add_entry: Option<AddEntryState>,
    quick_copy: Option<QuickCopyState>,
    pending_editor: Option<EditorRequest>,
    tmux_pane_opener: fn(&str, &Path, Option<usize>) -> Result<()>,
    should_quit: bool,
}

impl App {
    pub fn new(config: RuntimeConfig, engine: SearchEngine, initial_query: String) -> Self {
        let mut library_scopes = vec![LibraryScope::All];
        for library in &config.libraries {
            if library.pinned {
                library_scopes.push(LibraryScope::Alias(library.alias.clone()));
            }
        }
        let mut app = Self {
            config,
            engine,
            query_cursor: initial_query.len(),
            query: initial_query,
            filter: SourceFilter::All,
            mode: MatchMode::Fuzzy,
            focus: Focus::Results,
            previous_focus: Focus::Results,
            results: Vec::new(),
            selected: 0,
            preview: PreviewState::default(),
            preview_area: None,
            status: String::new(),
            library_scopes,
            library_index: 0,
            library_selector_index: 0,
            result_refine: None,
            add_entry: None,
            quick_copy: None,
            pending_editor: None,
            tmux_pane_opener: open_editor_in_tmux_pane,
            should_quit: false,
        };
        app.refresh_results();
        app
    }

    fn current_library_scope(&self) -> LibraryScope {
        self.library_scopes
            .get(self.library_index)
            .cloned()
            .unwrap_or(LibraryScope::All)
    }

    fn refresh_results(&mut self) {
        if self.result_refine.is_some() {
            self.refresh_refined_results();
            return;
        }
        self.refresh_indexed_results();
    }

    fn refresh_indexed_results(&mut self) {
        self.clamp_query_cursor();
        let request = SearchRequest {
            query: self.query.clone(),
            filter: self.filter,
            mode: self.mode,
            library: self.current_library_scope(),
            limit: 80,
        };
        self.results = self.engine.search(&request);
        self.finish_result_refresh();
    }

    fn refresh_refine_base_results(&mut self) {
        let Some(state) = &self.result_refine else {
            self.refresh_indexed_results();
            return;
        };
        let request = SearchRequest {
            query: state.base_query.clone(),
            filter: self.filter,
            mode: self.mode,
            library: self.current_library_scope(),
            limit: 80,
        };
        let base_results = self.engine.search(&request);
        if let Some(state) = &mut self.result_refine {
            state.base_results = base_results;
        }
        self.refresh_refined_results();
    }

    fn refresh_refined_results(&mut self) {
        self.clamp_query_cursor();
        let Some(state) = &self.result_refine else {
            return;
        };
        self.results = fuzzy_refine_results(&state.base_results, &self.query);
        self.finish_result_refresh();
    }

    fn finish_result_refresh(&mut self) {
        if self.selected >= self.results.len() {
            self.selected = self.results.len().saturating_sub(1);
        }
        self.load_selected_preview();
    }

    fn load_selected_preview(&mut self) {
        let Some(result) = self.results.get(self.selected) else {
            self.preview = PreviewState::default();
            return;
        };
        if self.preview.path.as_ref() == Some(&result.path) {
            self.preview.cursor = result_line_to_cursor(result.line, self.preview.lines.len());
            self.reposition_preview_scroll();
            return;
        }
        let text = fs::read_to_string(&result.path).unwrap_or_default();
        let lines = text.lines().map(str::to_string).collect::<Vec<_>>();
        let syntax_lines =
            match load_syntax_preview_lines(&result.path, lines.len(), &self.config.app.ui) {
                Ok(lines) => lines.unwrap_or_default(),
                Err(err) => {
                    if self.config.app.ui.preview_syntax == PreviewSyntax::Bat {
                        self.status = format!("bat preview failed: {err}");
                    }
                    Vec::new()
                }
            };
        let cursor = result_line_to_cursor(result.line, lines.len());
        let viewport_height = self.preview.viewport_height;
        self.preview = PreviewState {
            path: Some(result.path.clone()),
            lines,
            syntax_lines,
            scroll: 0,
            cursor,
            viewport_height,
            copy_mode: false,
            anchor: None,
        };
        self.reposition_preview_scroll();
    }

    fn select_next(&mut self) {
        if self.selected + 1 < self.results.len() {
            self.selected += 1;
            self.load_selected_preview();
        }
    }

    fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.load_selected_preview();
        }
    }

    fn preview_down(&mut self) {
        if self.preview.cursor + 1 < self.preview.lines.len() {
            self.preview.cursor += 1;
            self.reposition_preview_scroll();
        }
    }

    fn preview_up(&mut self) {
        if self.preview.cursor > 0 {
            self.preview.cursor -= 1;
            self.reposition_preview_scroll();
        }
    }

    fn preview_page_down(&mut self) {
        let step = self.preview.viewport_height.max(1) as isize;
        self.scroll_preview_view(step);
    }

    fn preview_page_up(&mut self) {
        let step = self.preview.viewport_height.max(1) as isize;
        self.scroll_preview_view(-step);
    }

    fn preview_line_scroll_down(&mut self) {
        self.scroll_preview_view(1);
    }

    fn preview_line_scroll_up(&mut self) {
        self.scroll_preview_view(-1);
    }

    fn preview_mouse_scroll_down(&mut self) {
        self.scroll_preview_view(MOUSE_SCROLL_LINES);
    }

    fn preview_mouse_scroll_up(&mut self) {
        self.scroll_preview_view(-MOUSE_SCROLL_LINES);
    }

    fn scroll_preview_view(&mut self, lines: isize) {
        if self.preview.lines.is_empty() {
            self.preview.scroll = 0;
            return;
        }
        let max_scroll = self
            .preview
            .lines
            .len()
            .saturating_sub(self.preview.viewport_height.max(1));
        if lines.is_negative() {
            self.preview.scroll = self.preview.scroll.saturating_sub(lines.unsigned_abs());
        } else {
            self.preview.scroll = self
                .preview
                .scroll
                .saturating_add(lines as usize)
                .min(max_scroll);
        }
    }

    fn set_preview_viewport_height(&mut self, height: usize) {
        if self.preview.viewport_height == height {
            return;
        }
        self.preview.viewport_height = height;
        self.reposition_preview_scroll();
    }

    fn reposition_preview_scroll(&mut self) {
        if self.preview.lines.is_empty() {
            self.preview.scroll = 0;
            self.preview.cursor = 0;
            self.preview.anchor = None;
            return;
        }
        let last_line = self.preview.lines.len() - 1;
        self.preview.cursor = self.preview.cursor.min(last_line);
        self.preview.anchor = self.preview.anchor.map(|anchor| anchor.min(last_line));
        let height = self.preview.viewport_height.max(1);
        let max_scroll = self.preview.lines.len().saturating_sub(height);
        let target_offset =
            preview_cursor_offset(height, self.config.app.ui.preview_cursor_percent);
        self.preview.scroll = self
            .preview
            .cursor
            .saturating_sub(target_offset)
            .min(max_scroll);
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        if !self.mouse_can_scroll_preview() || !self.mouse_is_over_preview(mouse.column, mouse.row)
        {
            return;
        }
        match mouse.kind {
            MouseEventKind::ScrollUp => self.preview_mouse_scroll_up(),
            MouseEventKind::ScrollDown => self.preview_mouse_scroll_down(),
            _ => {}
        }
    }

    fn mouse_can_scroll_preview(&self) -> bool {
        !matches!(
            self.focus,
            Focus::LibrarySelector | Focus::QuickCopy | Focus::Help
        )
    }

    fn mouse_is_over_preview(&self, column: u16, row: u16) -> bool {
        self.preview_area
            .is_some_and(|area| rect_contains(area, column, row))
    }

    fn toggle_library_scope(&mut self) {
        if self.library_scopes.len() <= 1 {
            self.focus = Focus::LibrarySelector;
            return;
        }
        if self.library_index + 1 < self.library_scopes.len() {
            self.library_index += 1;
            self.refresh_after_search_context_change();
        } else {
            self.focus = Focus::LibrarySelector;
            self.library_selector_index = 0;
        }
    }

    fn choose_library_from_selector(&mut self) {
        let scope = if self.library_selector_index == 0 {
            LibraryScope::All
        } else if let Some(library) = self
            .config
            .libraries
            .get(self.library_selector_index.saturating_sub(1))
        {
            LibraryScope::Alias(library.alias.clone())
        } else {
            return;
        };

        if let Some(index) = self.library_scopes.iter().position(|item| item == &scope) {
            self.library_index = index;
        } else {
            self.library_scopes.push(scope);
            self.library_index = self.library_scopes.len() - 1;
        }
        self.focus = Focus::Results;
        self.refresh_after_search_context_change();
    }

    fn library_selector_len(&self) -> usize {
        self.config.libraries.len() + 1
    }

    fn clamp_library_selector(&mut self) {
        self.library_selector_index = self
            .library_selector_index
            .min(self.library_selector_len().saturating_sub(1));
    }

    fn library_selector_next(&mut self) {
        self.clamp_library_selector();
        if self.library_selector_index + 1 < self.library_selector_len() {
            self.library_selector_index += 1;
        }
    }

    fn library_selector_prev(&mut self) {
        self.library_selector_index = self.library_selector_index.saturating_sub(1);
    }

    fn copy_preview_selection(&mut self) {
        if self.preview.lines.is_empty() {
            return;
        }
        let last_line = self.preview.lines.len() - 1;
        self.preview.cursor = self.preview.cursor.min(last_line);
        let start = self
            .preview
            .anchor
            .unwrap_or(self.preview.cursor)
            .min(last_line)
            .min(self.preview.cursor);
        let end = self
            .preview
            .anchor
            .unwrap_or(self.preview.cursor)
            .min(last_line)
            .max(self.preview.cursor);
        let text = self.preview.lines[start..=end].join("\n");
        match copy_text(&text, &self.config.app.clipboard.command) {
            Ok(()) => self.status = format!("copied lines {}-{}", start + 1, end + 1),
            Err(err) => self.status = format!("copy failed: {err}"),
        }
        self.preview.copy_mode = false;
        self.preview.anchor = None;
    }

    fn begin_quick_copy(&mut self) {
        let Some(result) = self.results.get(self.selected) else {
            self.status = "quick-copy failed: no result selected".to_string();
            return;
        };
        if result.source_kind != "note"
            || result.path.extension().and_then(|ext| ext.to_str()) != Some("md")
        {
            self.status = "quick-copy supports markdown notes only".to_string();
            return;
        }

        let text = match fs::read_to_string(&result.path) {
            Ok(text) => text,
            Err(err) => {
                self.status = format!("quick-copy failed: {err}");
                return;
            }
        };
        match quick_copy::extract(&text) {
            Ok(ExtractedCopy::Payload(text)) => self.copy_quick_copy_text(&text, "quick-copy"),
            Ok(ExtractedCopy::CodeBlocks(blocks)) => {
                self.quick_copy = Some(QuickCopyState {
                    blocks,
                    selected: 0,
                    return_focus: self.focus,
                });
                self.focus = Focus::QuickCopy;
                self.status =
                    "quick-copy: choose block, 1-9 copy, Enter copy, Esc cancel".to_string();
            }
            Err(err) => {
                self.status = format!("quick-copy failed: {err}");
            }
        }
    }

    fn close_quick_copy(&mut self) {
        let return_focus = self
            .quick_copy
            .as_ref()
            .map(|state| state.return_focus)
            .unwrap_or(Focus::Results);
        self.quick_copy = None;
        self.focus = return_focus;
    }

    fn quick_copy_next(&mut self) {
        let Some(state) = &mut self.quick_copy else {
            return;
        };
        if state.selected + 1 < state.blocks.len() {
            state.selected += 1;
        }
    }

    fn quick_copy_prev(&mut self) {
        let Some(state) = &mut self.quick_copy else {
            return;
        };
        state.selected = state.selected.saturating_sub(1);
    }

    fn copy_selected_quick_copy_block(&mut self) {
        let Some(state) = &self.quick_copy else {
            return;
        };
        self.copy_quick_copy_block(state.selected);
    }

    fn copy_quick_copy_block(&mut self, index: usize) {
        let Some(state) = &self.quick_copy else {
            return;
        };
        let Some(block) = state.blocks.get(index) else {
            return;
        };
        let text = block.body.clone();
        self.close_quick_copy();
        self.copy_quick_copy_text(&text, &format!("code block {}", index + 1));
    }

    fn copy_quick_copy_text(&mut self, text: &str, label: &str) {
        match copy_text(text, &self.config.app.clipboard.command) {
            Ok(()) => self.status = format!("copied {label}"),
            Err(err) => self.status = format!("copy failed: {err}"),
        }
    }

    fn toggle_help(&mut self) {
        if self.focus == Focus::Help {
            self.focus = self.previous_focus;
        } else {
            self.previous_focus = self.focus;
            self.focus = Focus::Help;
        }
    }

    fn close_help(&mut self) {
        if self.focus == Focus::Help {
            self.focus = self.previous_focus;
        }
    }

    fn toggle_query_mode(&mut self) {
        self.mode = match self.mode {
            MatchMode::Fuzzy => MatchMode::Exact,
            MatchMode::Exact => MatchMode::Fuzzy,
        };
        self.refresh_after_search_context_change();
    }

    fn refresh_after_search_context_change(&mut self) {
        if self.result_refine.is_some() {
            self.refresh_refine_base_results();
        } else {
            self.refresh_results();
        }
    }

    fn toggle_result_refine(&mut self) {
        if let Some(state) = self.result_refine.take() {
            self.query = state.base_query;
            self.query_cursor = state.base_query_cursor.min(self.query.len());
            self.results = state.base_results;
            self.status = "result filter off".to_string();
            self.finish_result_refresh();
            return;
        }

        self.clamp_query_cursor();
        let base_query = self.query.clone();
        let base_query_cursor = self.query_cursor;
        let base_count = self.results.len();
        self.result_refine = Some(ResultRefineState {
            base_query,
            base_query_cursor,
            base_results: self.results.clone(),
        });
        self.query.clear();
        self.query_cursor = 0;
        self.status = format!("result filter: fuzzy over {base_count} results; Ctrl-F return");
        self.refresh_results();
    }

    fn begin_add_entry(&mut self) {
        let note_query = self.note_query_for_new_entry();
        match note::infer_note_path(&self.config.libraries, &note_query) {
            Ok(path) => {
                let path_text = path.display().to_string();
                self.add_entry = Some(AddEntryState {
                    note_query,
                    path_cursor: path_text.len(),
                    path_text,
                });
                self.focus = Focus::AddEntry;
                self.status = "add entry: edit path, Enter open template, Esc cancel".to_string();
            }
            Err(err) => {
                self.status = format!("add failed: {err:#}");
            }
        }
    }

    fn note_query_for_new_entry(&self) -> String {
        self.result_refine
            .as_ref()
            .map(|state| state.base_query.clone())
            .unwrap_or_else(|| self.query.clone())
    }

    fn cancel_add_entry(&mut self) {
        self.add_entry = None;
        self.focus = Focus::Results;
        self.status = "add entry cancelled".to_string();
    }

    fn confirm_add_entry(&mut self) {
        let Some(state) = &self.add_entry else {
            return;
        };
        let path_text = state.path_text.trim();
        if path_text.is_empty() {
            self.status = "add failed: path is empty".to_string();
            return;
        }
        let path = expand_tilde_path(path_text);
        match note::create_note(&path, &state.note_query) {
            Ok(()) => {
                self.add_entry = None;
                self.focus = Focus::Results;
                self.status = format!("opening {}", path.display());
                self.pending_editor = Some(EditorRequest { path, line: None });
            }
            Err(err) => {
                self.status = format!("add failed: {err:#}");
            }
        }
    }

    fn clamp_add_path_cursor(&mut self) {
        let Some(state) = &mut self.add_entry else {
            return;
        };
        state.path_cursor = state.path_cursor.min(state.path_text.len());
        while !state.path_text.is_char_boundary(state.path_cursor) {
            state.path_cursor -= 1;
        }
    }

    fn insert_add_path_char(&mut self, ch: char) {
        self.clamp_add_path_cursor();
        let Some(state) = &mut self.add_entry else {
            return;
        };
        state.path_text.insert(state.path_cursor, ch);
        state.path_cursor += ch.len_utf8();
    }

    fn backspace_add_path_char(&mut self) {
        self.clamp_add_path_cursor();
        let Some(state) = &mut self.add_entry else {
            return;
        };
        let Some((start, _)) = previous_char(&state.path_text, state.path_cursor) else {
            return;
        };
        state.path_text.replace_range(start..state.path_cursor, "");
        state.path_cursor = start;
    }

    fn delete_previous_add_path_word(&mut self) {
        self.clamp_add_path_cursor();
        let Some(state) = &mut self.add_entry else {
            return;
        };
        let mut start = state.path_cursor;
        while let Some((idx, ch)) = previous_char(&state.path_text, start) {
            if !ch.is_whitespace() && ch != '/' {
                break;
            }
            start = idx;
        }
        while let Some((idx, ch)) = previous_char(&state.path_text, start) {
            if ch.is_whitespace() || ch == '/' {
                break;
            }
            start = idx;
        }
        if start == state.path_cursor {
            return;
        }
        state.path_text.replace_range(start..state.path_cursor, "");
        state.path_cursor = start;
    }

    fn move_add_path_left(&mut self) {
        self.clamp_add_path_cursor();
        let Some(state) = &mut self.add_entry else {
            return;
        };
        if let Some((idx, _)) = previous_char(&state.path_text, state.path_cursor) {
            state.path_cursor = idx;
        }
    }

    fn move_add_path_right(&mut self) {
        self.clamp_add_path_cursor();
        let Some(state) = &mut self.add_entry else {
            return;
        };
        if let Some((idx, ch)) = next_char(&state.path_text, state.path_cursor) {
            state.path_cursor = idx + ch.len_utf8();
        }
    }

    fn clamp_query_cursor(&mut self) {
        self.query_cursor = self.query_cursor.min(self.query.len());
        while !self.query.is_char_boundary(self.query_cursor) {
            self.query_cursor -= 1;
        }
    }

    fn insert_query_char(&mut self, ch: char) {
        self.clamp_query_cursor();
        self.query.insert(self.query_cursor, ch);
        self.query_cursor += ch.len_utf8();
        self.refresh_results();
    }

    fn backspace_query_char(&mut self) {
        self.clamp_query_cursor();
        let Some((start, _)) = previous_char(&self.query, self.query_cursor) else {
            return;
        };
        self.query.replace_range(start..self.query_cursor, "");
        self.query_cursor = start;
        self.refresh_results();
    }

    fn delete_previous_query_word(&mut self) {
        self.clamp_query_cursor();
        let mut start = self.query_cursor;
        while let Some((idx, ch)) = previous_char(&self.query, start) {
            if !ch.is_whitespace() {
                break;
            }
            start = idx;
        }
        while let Some((idx, ch)) = previous_char(&self.query, start) {
            if ch.is_whitespace() {
                break;
            }
            start = idx;
        }
        if start == self.query_cursor {
            return;
        }
        self.query.replace_range(start..self.query_cursor, "");
        self.query_cursor = start;
        self.refresh_results();
    }

    fn handle_results_control_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('w') | KeyCode::Char('W') => self.delete_previous_query_word(),
            _ => return false,
        }
        true
    }

    fn request_open_selected_result(&mut self) {
        let Some(result) = self.results.get(self.selected) else {
            self.status = "no result selected".to_string();
            return;
        };
        self.pending_editor = Some(EditorRequest {
            path: result.path.clone(),
            line: (result.line > 0).then_some(result.line),
        });
    }

    fn request_open_selected_result_in_tmux_pane(&mut self) {
        let Some(result) = self.results.get(self.selected) else {
            self.status = "no result selected".to_string();
            return;
        };
        let path = result.path.clone();
        let line = (result.line > 0).then_some(result.line);
        match (self.tmux_pane_opener)(&self.config.app.editor.command, &path, line) {
            Ok(()) => {
                self.status = format!("opened {} in tmux pane", path.display());
                self.should_quit = true;
            }
            Err(err) => {
                self.status = format!("tmux pane open failed: {err:#}");
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('q') | KeyCode::Char('Q') => {
                    self.should_quit = true;
                    return Ok(());
                }
                KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Backspace => {
                    self.toggle_help();
                    return Ok(());
                }
                _ => {}
            }
        }

        if self.focus == Focus::Help {
            return self.handle_help_key(key);
        }

        if self.focus == Focus::AddEntry {
            return self.handle_add_entry_key(key);
        }

        if self.focus == Focus::QuickCopy {
            return self.handle_quick_copy_key(key);
        }

        if self.handle_preview_scroll_key(key) {
            return Ok(());
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            if self.focus == Focus::Results && self.handle_results_control_key(key) {
                return Ok(());
            }
            match key.code {
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    self.begin_add_entry();
                    return Ok(());
                }
                KeyCode::Char('c') | KeyCode::Char('C')
                    if matches!(self.focus, Focus::Results | Focus::Preview) =>
                {
                    self.begin_quick_copy();
                    return Ok(());
                }
                KeyCode::Char('o') | KeyCode::Char('O')
                    if matches!(self.focus, Focus::Results | Focus::Preview) =>
                {
                    self.request_open_selected_result_in_tmux_pane();
                    return Ok(());
                }
                KeyCode::Char('x') | KeyCode::Char('X') => {
                    self.toggle_query_mode();
                    return Ok(());
                }
                KeyCode::Char('t') | KeyCode::Char('T') => {
                    self.filter = self.filter.next();
                    self.refresh_after_search_context_change();
                    return Ok(());
                }
                KeyCode::Char('f') | KeyCode::Char('F') => {
                    self.toggle_result_refine();
                    return Ok(());
                }
                KeyCode::Char('l') | KeyCode::Char('L') => {
                    self.toggle_library_scope();
                    return Ok(());
                }
                _ => {}
            }
        }

        match self.focus {
            Focus::Results => self.handle_results_key(key),
            Focus::Preview => self.handle_preview_key(key),
            Focus::LibrarySelector => self.handle_library_selector_key(key),
            Focus::AddEntry => self.handle_add_entry_key(key),
            Focus::QuickCopy => self.handle_quick_copy_key(key),
            Focus::Help => self.handle_help_key(key),
        }
    }

    fn handle_results_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Tab => self.focus = Focus::Preview,
            KeyCode::Enter => self.request_open_selected_result(),
            KeyCode::Up => self.select_prev(),
            KeyCode::Down => self.select_next(),
            KeyCode::Backspace => self.backspace_query_char(),
            KeyCode::Char(ch) => self.insert_query_char(ch),
            _ => {}
        }
        Ok(())
    }

    fn handle_add_entry_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('w') | KeyCode::Char('W') => self.delete_previous_add_path_word(),
                _ => {}
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => self.cancel_add_entry(),
            KeyCode::Enter => self.confirm_add_entry(),
            KeyCode::Left => self.move_add_path_left(),
            KeyCode::Right => self.move_add_path_right(),
            KeyCode::Backspace => self.backspace_add_path_char(),
            KeyCode::Char(ch) => self.insert_add_path_char(ch),
            _ => {}
        }
        Ok(())
    }

    fn handle_preview_scroll_key(&mut self, key: KeyEvent) -> bool {
        if !matches!(self.focus, Focus::Results | Focus::Preview) {
            return false;
        }
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return false;
        }
        match key.code {
            KeyCode::PageUp => self.preview_page_up(),
            KeyCode::PageDown => self.preview_page_down(),
            KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.preview_line_scroll_up();
            }
            KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.preview_line_scroll_down();
            }
            _ => return false,
        }
        true
    }

    fn handle_preview_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                if self.preview.copy_mode {
                    self.preview.copy_mode = false;
                    self.preview.anchor = None;
                } else {
                    self.focus = Focus::Results;
                }
            }
            KeyCode::Tab => self.focus = Focus::Results,
            KeyCode::Up | KeyCode::Char('k') => self.preview_up(),
            KeyCode::Down | KeyCode::Char('j') => self.preview_down(),
            KeyCode::Char('v') if self.preview.copy_mode => {
                self.preview.anchor = Some(self.preview.cursor);
                self.status = format!("selection anchor: line {}", self.preview.cursor + 1);
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if self.preview.copy_mode {
                    self.copy_preview_selection();
                } else {
                    self.preview.copy_mode = true;
                    self.preview.anchor = Some(self.preview.cursor);
                    self.status = "copy mode: move to select, Enter copy, Esc cancel".to_string();
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_library_selector_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Tab => self.focus = Focus::Results,
            KeyCode::Enter => self.choose_library_from_selector(),
            KeyCode::Up | KeyCode::Char('k') => self.library_selector_prev(),
            KeyCode::Down | KeyCode::Char('j') => self.library_selector_next(),
            _ => {}
        }
        Ok(())
    }

    fn handle_quick_copy_key(&mut self, key: KeyEvent) -> Result<()> {
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return Ok(());
        }
        match key.code {
            KeyCode::Esc | KeyCode::Tab => {
                self.close_quick_copy();
                self.status = "quick-copy cancelled".to_string();
            }
            KeyCode::Enter | KeyCode::Char(' ') => self.copy_selected_quick_copy_block(),
            KeyCode::Up | KeyCode::Char('k') => self.quick_copy_prev(),
            KeyCode::Down | KeyCode::Char('j') => self.quick_copy_next(),
            KeyCode::Char(ch) => {
                if let Some(index) = ch
                    .to_digit(10)
                    .and_then(|digit| usize::try_from(digit).ok())
                    .and_then(|digit| digit.checked_sub(1))
                    .filter(|index| *index < 9)
                {
                    self.copy_quick_copy_block(index);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_help_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc
            | KeyCode::Tab
            | KeyCode::Enter
            | KeyCode::Char(' ')
            | KeyCode::Char('q')
            | KeyCode::Char('Q') => self.close_help(),
            _ => {}
        }
        Ok(())
    }
}

pub fn run(config: RuntimeConfig, engine: SearchEngine, initial_query: String) -> Result<()> {
    let mut terminal = init_terminal()?;
    let result = run_inner(&mut terminal, App::new(config, engine, initial_query));
    restore_terminal();
    result
}

fn run_inner(terminal: &mut DefaultTerminal, mut app: App) -> Result<()> {
    loop {
        terminal.draw(|frame| draw(frame, &mut app))?;
        if app.should_quit {
            return Ok(());
        }
        if event::poll(Duration::from_millis(80))? {
            match event::read()? {
                Event::Key(key) => app.handle_key(key)?,
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                _ => {}
            }
            if let Some(request) = app.pending_editor.take() {
                handle_editor_request(terminal, &mut app, request)?;
                if app.should_quit {
                    return Ok(());
                }
            }
        }
    }
}

fn init_terminal() -> Result<DefaultTerminal> {
    let terminal = ratatui::init();
    if let Err(err) = enable_mouse_capture() {
        ratatui::restore();
        return Err(err);
    }
    Ok(terminal)
}

fn restore_terminal() {
    let _ = disable_mouse_capture();
    ratatui::restore();
}

fn enable_mouse_capture() -> Result<()> {
    let mut stdout = io::stdout();
    execute!(stdout, EnableMouseCapture)?;
    Ok(())
}

fn disable_mouse_capture() -> Result<()> {
    let mut stdout = io::stdout();
    execute!(stdout, DisableMouseCapture)?;
    Ok(())
}

fn handle_editor_request(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    request: EditorRequest,
) -> Result<()> {
    restore_terminal();
    let editor_result = open_editor(&app.config.app.editor.command, &request.path, request.line);

    match app.config.app.editor.return_behavior {
        EditorReturn::Exit => {
            app.should_quit = true;
            editor_result?;
        }
        EditorReturn::Resume => {
            *terminal = init_terminal()?;
            match editor_result {
                Ok(status) => match refresh_after_editor(app) {
                    Ok(()) if status.success() => {
                        app.status = "returned from editor".to_string();
                    }
                    Ok(()) => {
                        app.status = editor_status_message(status);
                    }
                    Err(err) => {
                        app.status = format!("refresh failed: {err:#}");
                    }
                },
                Err(err) => {
                    app.status = format!("editor failed: {err}");
                }
            }
        }
    }
    Ok(())
}

fn refresh_after_editor(app: &mut App) -> Result<()> {
    ensure_indexes(&app.config.libraries, false, |_library, _stage| {})?;
    let manager = IndexManager::open(&app.config.libraries)?;
    app.engine = SearchEngine::new(manager);
    app.preview.path = None;
    app.refresh_after_search_context_change();
    Ok(())
}

fn editor_status_message(status: std::process::ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("editor exited with status {code}"),
        None => "editor exited without a status code".to_string(),
    }
}

fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);
    draw_preview(frame, chunks[0], app);
    draw_results(frame, chunks[1], app);
    if app.focus == Focus::LibrarySelector {
        draw_library_selector(frame, centered_rect(70, 70, area), app);
    }
    if app.focus == Focus::QuickCopy {
        draw_quick_copy(frame, centered_rect(82, 64, area), app);
    }
    if app.focus == Focus::Help {
        draw_help(frame, centered_rect(78, 78, area));
    }
}

fn draw_preview(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    app.preview_area = Some(area);
    let title = match app.focus {
        Focus::Preview if app.preview.copy_mode => " Preview [copy] ",
        Focus::Preview => " Preview [focused] ",
        _ => " Preview ",
    };
    let visible_height = area.height.saturating_sub(2) as usize;
    app.set_preview_viewport_height(visible_height);
    let start = app
        .preview
        .scroll
        .min(app.preview.lines.len().saturating_sub(1));
    let end = (start + visible_height).min(app.preview.lines.len());
    let anchor = app.preview.anchor.unwrap_or(app.preview.cursor);
    let range_start = anchor.min(app.preview.cursor);
    let range_end = anchor.max(app.preview.cursor);
    let terms = highlight_terms(&app.query);
    let mut items = Vec::new();
    for (idx, line) in app.preview.lines[start..end].iter().enumerate() {
        let absolute = start + idx;
        let syntax_line = app.preview.syntax_lines.get(absolute);
        let line_has_match = line_has_highlight(line, &terms);
        let is_copy_selected = app.focus == Focus::Preview
            && app.preview.copy_mode
            && absolute >= range_start
            && absolute <= range_end;
        let is_cursor = app.focus == Focus::Preview && absolute == app.preview.cursor;
        items.push(ListItem::new(highlighted_preview_line(
            absolute + 1,
            line,
            syntax_line,
            &terms,
            line_has_match,
            is_cursor,
            is_copy_selected,
        )));
    }
    if items.is_empty() {
        items.push(ListItem::new("No preview"));
    }
    let list = List::new(items).block(Block::new().borders(Borders::BOTTOM).title(title));
    frame.render_widget(list, area);
}

fn draw_results(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(area);
    let prompt = prompt_line(app);
    frame.render_widget(Paragraph::new(prompt), chunks[0]);
    if matches!(app.focus, Focus::Results | Focus::AddEntry) {
        frame.set_cursor_position(prompt_cursor_position(app, chunks[0]));
    }
    let header = if app.status.is_empty() {
        match app.focus {
            Focus::AddEntry => "Enter create/open | Esc cancel | Left/Right edit path | C-Q quit",
            Focus::Preview => {
                "Tab results | Enter copy | C-C copy | C-A add | C-X exact | C-F filter | C-T type | C-L lib | C-H help"
            }
            Focus::QuickCopy => "1-9 copy | Enter copy | Up/Down select | Esc cancel | C-Q quit",
            _ => {
                "Tab preview | Enter open | C-C copy | C-A add | C-X exact | C-F filter | C-T type | C-L lib | C-H help"
            }
        }
        .to_string()
    } else {
        app.status.clone()
    };
    frame.render_widget(
        Paragraph::new(header).style(Style::default().fg(Color::DarkGray)),
        chunks[1],
    );
    let items = app
        .results
        .iter()
        .map(|result| ListItem::new(highlighted_result_line(result, &app.query)))
        .collect::<Vec<_>>();
    let mut state = ListState::default();
    if !app.results.is_empty() {
        state.select(Some(app.selected));
    }
    let list = List::new(items)
        .block(Block::default())
        .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
        .highlight_symbol("> ");
    frame.render_stateful_widget(list, chunks[2], &mut state);
}

#[cfg(test)]
fn prompt_text(app: &App) -> String {
    let (prefix, text, _) = prompt_parts(app);
    format!("{prefix}{text}")
}

fn prompt_line(app: &App) -> Line<'static> {
    let colors = &app.config.app.ui.prompt.colors;
    if let Some(add_entry) = &app.add_entry {
        let right_separator = prompt_right_separator(app);
        return Line::from(vec![
            Span::styled("add", prompt_color_style(&colors.add, Color::Cyan)),
            Span::styled(
                format!("{right_separator} "),
                prompt_color_style(&colors.marker, Color::DarkGray),
            ),
            Span::styled(
                add_entry.path_text.clone(),
                prompt_color_style(&colors.query, Color::White),
            ),
        ]);
    }

    let mode = match app.mode {
        MatchMode::Exact => ("X", &colors.exact, Color::Red),
        MatchMode::Fuzzy => ("F", &colors.fuzzy, Color::Cyan),
    };
    let separator = prompt_separator(app);
    let scope = app.current_library_scope().label().to_string();
    let mut spans = vec![
        Span::styled(mode.0, prompt_color_style(mode.1, mode.2)),
        Span::styled(
            separator.clone(),
            prompt_color_style(&colors.separator, Color::DarkGray),
        ),
        Span::styled(scope, prompt_color_style(&colors.scope, Color::Blue)),
    ];
    if let Some(filter) = app.filter.label() {
        spans.push(Span::styled(
            separator,
            prompt_color_style(&colors.separator, Color::DarkGray),
        ));
        spans.push(Span::styled(
            filter,
            prompt_color_style(&colors.filter, Color::Green),
        ));
    }
    spans.push(Span::styled(
        format!("{} ", prompt_right_separator(app)),
        prompt_color_style(&colors.marker, Color::DarkGray),
    ));
    spans.push(Span::styled(
        app.query.clone(),
        prompt_color_style(&colors.query, Color::White),
    ));
    Line::from(spans)
}

fn prompt_parts(app: &App) -> (String, String, usize) {
    if let Some(add_entry) = &app.add_entry {
        return (
            "add> ".to_string(),
            add_entry.path_text.clone(),
            add_entry.path_cursor,
        );
    }

    let mode = match app.mode {
        MatchMode::Exact => "X",
        MatchMode::Fuzzy => "F",
    };
    let scope = app.current_library_scope();
    let separator = prompt_separator(app);
    let mut prefix = format!("{mode}{separator}{}", scope.label());
    if let Some(filter) = app.filter.label() {
        prefix.push_str(&separator);
        prefix.push_str(filter);
    }
    (
        format!("{prefix}{} ", prompt_right_separator(app)),
        app.query.clone(),
        app.query_cursor,
    )
}

fn prompt_separator(app: &App) -> String {
    let separator = app.config.app.ui.prompt.separator.trim();
    if separator.is_empty() {
        ":".to_string()
    } else {
        separator.to_string()
    }
}

fn prompt_right_separator(app: &App) -> String {
    let separator = app.config.app.ui.prompt.right_separator.trim();
    if separator.is_empty() {
        ">".to_string()
    } else {
        separator.to_string()
    }
}

fn prompt_color_style(value: &str, fallback: Color) -> Style {
    Style::default().fg(prompt_color(value, fallback))
}

fn prompt_color(value: &str, fallback: Color) -> Color {
    match value.trim().to_ascii_lowercase().as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "dark-gray" | "dark-grey" | "darkgray" | "darkgrey" => Color::DarkGray,
        "white" => Color::White,
        "reset" | "default" => Color::Reset,
        _ => fallback,
    }
}

fn prompt_cursor_position(app: &App, area: Rect) -> (u16, u16) {
    let (prefix, text, cursor) = prompt_parts(app);
    let cursor = cursor.min(text.len());
    let cursor = if text.is_char_boundary(cursor) {
        cursor
    } else {
        text.char_indices()
            .map(|(idx, _)| idx)
            .take_while(|idx| *idx < cursor)
            .last()
            .unwrap_or(0)
    };
    let cells = prefix.chars().count() + text[..cursor].chars().count();
    let max_x = area.width.saturating_sub(1) as usize;
    (area.x + cells.min(max_x) as u16, area.y)
}

fn previous_char(value: &str, cursor: usize) -> Option<(usize, char)> {
    value[..cursor].char_indices().next_back()
}

fn next_char(value: &str, cursor: usize) -> Option<(usize, char)> {
    value[cursor..]
        .char_indices()
        .next()
        .map(|(idx, ch)| (cursor + idx, ch))
}

fn expand_tilde_path(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(path)
}

fn highlighted_result_line(result: &SearchResult, query: &str) -> Line<'static> {
    let text = format!("{}  {}", result.display_line(), result.title);
    let terms = highlight_terms(query);
    if terms.is_empty() {
        return Line::from(text);
    }

    Line::from(highlighted_text_spans(&text, &terms))
}

fn fuzzy_refine_results(candidates: &[SearchResult], query: &str) -> Vec<SearchResult> {
    let query = query.trim();
    if query.is_empty() {
        return candidates.to_vec();
    }

    let mut matcher = Matcher::new(FuzzyConfig::DEFAULT.match_paths());
    let pattern = Pattern::new(
        query,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
    );
    let mut buf = Vec::new();
    let mut scored = Vec::new();

    for (idx, result) in candidates.iter().enumerate() {
        let text = result_refine_text(result);
        let Some(score) = pattern.score(Utf32Str::new(&text, &mut buf), &mut matcher) else {
            continue;
        };
        let mut result = result.clone();
        let original_score = result.score;
        result.score = score as f32 + original_score * 0.001;
        result.rank_reason = "result-refine".to_string();
        scored.push((score, original_score, idx, result));
    }

    scored.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| {
                right
                    .1
                    .partial_cmp(&left.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| left.2.cmp(&right.2))
    });
    scored.into_iter().map(|(_, _, _, result)| result).collect()
}

fn result_refine_text(result: &SearchResult) -> String {
    format!(
        "{} {} {}",
        result.display_line(),
        result.title,
        result.snippet
    )
}

fn preview_cursor_offset(height: usize, percent: u8) -> usize {
    if height == 0 {
        return 0;
    }
    (height * usize::from(percent.min(100)) / 100).min(height.saturating_sub(1))
}

fn result_line_to_cursor(line: usize, line_count: usize) -> usize {
    if line_count == 0 {
        0
    } else {
        line.saturating_sub(1).min(line_count - 1)
    }
}

fn rect_contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x
        && column < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}

fn load_syntax_preview_lines(
    path: &Path,
    raw_line_count: usize,
    ui: &UiConfig,
) -> std::result::Result<Option<Vec<Line<'static>>>, String> {
    if ui.preview_syntax == PreviewSyntax::Plain {
        return Ok(None);
    }
    let Some(program) = resolve_bat_command() else {
        return if ui.preview_syntax == PreviewSyntax::Bat {
            Err("bat or batcat was not found on PATH".to_string())
        } else {
            Ok(None)
        };
    };
    run_bat_preview(&program, path, raw_line_count, &ui.preview_bat_args).map(Some)
}

fn resolve_bat_command() -> Option<PathBuf> {
    let path = env::var_os("PATH");
    resolve_bat_command_from_path(path.as_deref())
}

fn resolve_bat_command_from_path(path: Option<&OsStr>) -> Option<PathBuf> {
    ["bat", "batcat"]
        .into_iter()
        .find_map(|command| find_command_in_path(command, path))
}

fn find_command_in_path(command: &str, path: Option<&OsStr>) -> Option<PathBuf> {
    let path = path?;
    env::split_paths(path)
        .map(|dir| dir.join(command))
        .find(|candidate| candidate.is_file())
}

fn run_bat_preview(
    program: &Path,
    path: &Path,
    raw_line_count: usize,
    user_args: &[String],
) -> std::result::Result<Vec<Line<'static>>, String> {
    let output = Command::new(program)
        .args(bat_preview_args(path, user_args))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(format!("bat exited with {}", output.status));
    }
    parse_bat_preview_output(&output.stdout, raw_line_count)
}

fn bat_preview_args(path: &Path, user_args: &[String]) -> Vec<OsString> {
    let mut args = BAT_PREVIEW_ARGS
        .iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    args.extend(user_args.iter().map(OsString::from));
    args.push(OsString::from("--"));
    args.push(path.as_os_str().to_os_string());
    args
}

fn parse_bat_preview_output(
    bytes: &[u8],
    raw_line_count: usize,
) -> std::result::Result<Vec<Line<'static>>, String> {
    let text = bytes.into_text().map_err(|err| err.to_string())?;
    if text.lines.len() != raw_line_count {
        return Err(format!(
            "bat output line count {} did not match source line count {}",
            text.lines.len(),
            raw_line_count
        ));
    }
    Ok(text.lines)
}

fn highlighted_preview_line(
    line_number: usize,
    text: &str,
    syntax_line: Option<&Line<'_>>,
    terms: &[String],
    line_has_match: bool,
    is_cursor: bool,
    is_copy_selected: bool,
) -> Line<'static> {
    let number_style = preview_line_number_style(line_has_match, is_cursor, is_copy_selected);
    let mut spans = vec![Span::styled(format!("{line_number:>5} "), number_style)];
    if let Some(syntax_line) = syntax_line {
        spans.extend(highlighted_styled_spans(&syntax_line.spans, terms));
    } else {
        spans.extend(highlighted_text_spans(text, terms));
    }
    Line::from(spans)
}

fn preview_line_number_style(
    line_has_match: bool,
    is_cursor: bool,
    is_copy_selected: bool,
) -> Style {
    if is_copy_selected {
        return Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD);
    }
    if is_cursor {
        return Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
    }
    if line_has_match {
        return Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
    }
    Style::default().fg(Color::DarkGray)
}

fn highlighted_styled_spans(source: &[Span<'_>], terms: &[String]) -> Vec<Span<'static>> {
    if terms.is_empty() {
        return owned_spans(source);
    }

    let text = source
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();
    let lower = text.to_ascii_lowercase();
    let mut ranges = Vec::new();
    let mut cursor = 0;
    while cursor < text.len() {
        let Some((start, end)) = next_highlight_range(&lower, cursor, terms) else {
            break;
        };
        ranges.push((start, end));
        cursor = end;
    }
    if ranges.is_empty() {
        return owned_spans(source);
    }

    let mut spans = Vec::new();
    let mut range_idx = 0;
    let mut absolute = 0;
    for span in source {
        let content = span.content.as_ref();
        let span_start = absolute;
        let span_end = span_start + content.len();
        let mut local = 0;
        while local < content.len() {
            let position = span_start + local;
            while range_idx < ranges.len() && ranges[range_idx].1 <= position {
                range_idx += 1;
            }
            if range_idx < ranges.len()
                && ranges[range_idx].0 <= position
                && position < ranges[range_idx].1
            {
                let end = ranges[range_idx].1.min(span_end);
                let local_end = local + (end - position);
                spans.push(Span::styled(
                    content[local..local_end].to_string(),
                    query_match_style(span.style),
                ));
                local = local_end;
            } else {
                let end = ranges
                    .get(range_idx)
                    .map(|(start, _)| (*start).min(span_end))
                    .unwrap_or(span_end);
                let local_end = local + (end - position);
                spans.push(Span::styled(
                    content[local..local_end].to_string(),
                    span.style,
                ));
                local = local_end;
            }
        }
        absolute = span_end;
    }
    spans
}

fn owned_spans(source: &[Span<'_>]) -> Vec<Span<'static>> {
    source
        .iter()
        .map(|span| Span::styled(span.content.to_string(), span.style))
        .collect()
}

fn query_match_style(style: Style) -> Style {
    style.fg(Color::Yellow).add_modifier(Modifier::BOLD)
}

fn highlighted_text_spans(text: &str, terms: &[String]) -> Vec<Span<'static>> {
    if terms.is_empty() {
        return vec![Span::raw(text.to_string())];
    }

    let lower = text.to_ascii_lowercase();
    let mut spans = Vec::new();
    let mut cursor = 0;
    while cursor < text.len() {
        let Some((start, end)) = next_highlight_range(&lower, cursor, terms) else {
            spans.push(Span::raw(text[cursor..].to_string()));
            break;
        };
        if start > cursor {
            spans.push(Span::raw(text[cursor..start].to_string()));
        }
        spans.push(Span::styled(
            text[start..end].to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        cursor = end;
    }

    spans
}

fn line_has_highlight(text: &str, terms: &[String]) -> bool {
    !terms.is_empty() && next_highlight_range(&text.to_ascii_lowercase(), 0, terms).is_some()
}

fn highlight_terms(query: &str) -> Vec<String> {
    let mut terms = Vec::new();
    for source in [query.to_string(), normalize_query(query)] {
        for term in source.split(|ch: char| !ch.is_alphanumeric() && ch != '_') {
            if term.is_empty() {
                continue;
            }
            let term = term.to_ascii_lowercase();
            if !terms.contains(&term) {
                terms.push(term);
            }
        }
    }
    terms.sort_by_key(|term| std::cmp::Reverse(term.len()));
    terms
}

fn next_highlight_range(
    lower_text: &str,
    cursor: usize,
    terms: &[String],
) -> Option<(usize, usize)> {
    terms
        .iter()
        .filter_map(|term| {
            lower_text[cursor..]
                .find(term)
                .map(|offset| (cursor + offset, cursor + offset + term.len()))
        })
        .min_by(|(left_start, left_end), (right_start, right_end)| {
            left_start
                .cmp(right_start)
                .then_with(|| (right_end - right_start).cmp(&(left_end - left_start)))
        })
}

fn draw_library_selector(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    frame.render_widget(Clear, area);
    let mut items = vec![ListItem::new("all  all configured libraries")];
    items.extend(
        app.config
            .libraries
            .iter()
            .map(|library| ListItem::new(format!("{}  {}", library.alias, library.path.display()))),
    );
    let mut state = ListState::default();
    state.select(Some(app.library_selector_index));
    let list = List::new(items)
        .block(Block::bordered().title(" Libraries "))
        .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
        .highlight_symbol("> ");
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_quick_copy(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    frame.render_widget(Clear, area);
    let Some(state) = &app.quick_copy else {
        return;
    };
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(area);
    let items = state
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| ListItem::new(quick_copy_block_label(index, block)))
        .collect::<Vec<_>>();
    let mut list_state = ListState::default();
    list_state.select(Some(state.selected));
    let list = List::new(items)
        .block(Block::bordered().title(" Quick Copy "))
        .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
        .highlight_symbol("> ");
    frame.render_stateful_widget(list, chunks[0], &mut list_state);

    let selected = state.blocks.get(state.selected);
    let preview_title = selected
        .map(|block| {
            let language = if block.language.is_empty() {
                "text"
            } else {
                &block.language
            };
            format!(" Block {} ({language}) ", state.selected + 1)
        })
        .unwrap_or_else(|| " Block ".to_string());
    let preview = selected.map(|block| block.body.clone()).unwrap_or_default();
    let paragraph = Paragraph::new(preview)
        .block(Block::bordered().title(preview_title))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, chunks[1]);
}

fn quick_copy_block_label(index: usize, block: &CodeBlock) -> Line<'static> {
    let shortcut = if index < 9 {
        format!("{}", index + 1)
    } else {
        "-".to_string()
    };
    let language = if block.language.is_empty() {
        "text"
    } else {
        &block.language
    };
    let summary = quick_copy::first_non_empty_line(&block.body).unwrap_or("(empty)");
    Line::from(vec![
        Span::styled(
            format!("{shortcut:<2}"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("{language:<10}"), Style::default().fg(Color::Cyan)),
        Span::raw(summary.to_string()),
    ])
}

fn draw_help(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(Clear, area);
    let items = help_lines()
        .iter()
        .map(|(key, description)| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{key:<18}"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(*description),
            ]))
        })
        .collect::<Vec<_>>();
    let list = List::new(items)
        .block(Block::bordered().title(" Help "))
        .style(Style::default().fg(Color::White));
    frame.render_widget(list, area);
}

fn help_lines() -> &'static [(&'static str, &'static str)] {
    &[
        ("Ctrl-H", "open or close this help popup"),
        ("q/Enter/Space", "close this help popup"),
        ("Ctrl-Q", "quit from any mode"),
        ("Esc", "cancel popup/copy/focus; quit from results"),
        ("Tab", "switch results and preview focus; close popups"),
        ("Ctrl-A", "add a new library entry from the current query"),
        ("Ctrl-C", "quick-copy the selected note payload"),
        ("Ctrl-O", "open selected result in a new tmux pane"),
        ("Ctrl-W", "delete previous query word"),
        ("Ctrl-X", "toggle exact/fuzzy query mode"),
        ("Ctrl-F", "filter over current results"),
        ("Ctrl-T", "cycle result type: all, names, content, man"),
        ("Ctrl-L", "cycle pinned libraries or open library picker"),
        ("typing", "edit the query in results mode"),
        ("Backspace", "delete query text in results mode"),
        ("Up/Down", "move through results"),
        ("j/k", "move through preview lines or library picker"),
        ("PageUp/PageDown", "scroll preview by a page"),
        ("Shift-Up/Down", "scroll preview by one line"),
        (
            "Enter",
            "results: open selected result; picker: choose library",
        ),
        (
            "Enter/Space",
            "preview: start copy selection; copy mode: copy selected lines",
        ),
        (
            "v",
            "copy mode: move selection anchor to current preview line",
        ),
        ("1-9", "quick-copy chooser: copy numbered code block"),
        (
            "all",
            "first library picker row searches all configured libraries",
        ),
    ]
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

#[cfg(test)]
#[path = "../tests/unit/tui.rs"]
mod tests;
