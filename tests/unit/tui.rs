use super::*;
use crate::config::{AppConfig, ClipboardConfig, EditorConfig, UiConfig};
use crate::library::Library;
use std::sync::Mutex;

#[derive(Clone, Debug, Eq, PartialEq)]
struct TmuxOpenCall {
    editor: String,
    path: PathBuf,
    line: Option<usize>,
}

static TMUX_OPEN_CALL: Mutex<Option<TmuxOpenCall>> = Mutex::new(None);

fn test_app() -> App {
    test_app_with_libraries(Vec::new())
}

fn test_app_with_libraries(libraries: Vec<Library>) -> App {
    let config = RuntimeConfig {
        path: None,
        app: AppConfig {
            libraries: Vec::new(),
            editor: EditorConfig {
                command: "true".to_string(),
                return_behavior: EditorReturn::Resume,
            },
            clipboard: ClipboardConfig::default(),
            ui: UiConfig::default(),
        },
        libraries,
    };
    let engine = SearchEngine::new(IndexManager {
        handles: Vec::new(),
    });
    App::new(config, engine, String::new())
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl_key(ch: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(ch), KeyModifiers::CONTROL)
}

fn shift_key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::SHIFT)
}

fn record_tmux_pane_open(editor: &str, path: &Path, line: Option<usize>) -> Result<()> {
    *TMUX_OPEN_CALL.lock().unwrap() = Some(TmuxOpenCall {
        editor: editor.to_string(),
        path: path.to_path_buf(),
        line,
    });
    Ok(())
}

fn fail_tmux_pane_open(_: &str, _: &Path, _: Option<usize>) -> Result<()> {
    anyhow::bail!("tmux missing")
}

fn search_result(path: &str, line: usize) -> SearchResult {
    search_result_with_title(path, line, "Example")
}

fn search_result_with_title(path: &str, line: usize, title: &str) -> SearchResult {
    SearchResult {
        title: title.to_string(),
        path: PathBuf::from(path),
        rel_path: format!("{}.md", title.to_ascii_lowercase().replace(' ', "-")),
        library_alias: "test".to_string(),
        source_kind: "note".to_string(),
        line,
        snippet: String::new(),
        score: 1.0,
        rank_reason: "test".to_string(),
        body: String::new(),
        is_live_man: false,
    }
}

#[test]
fn bat_preview_args_append_user_args_before_path_separator() {
    let args = bat_preview_args(
        Path::new("/tmp/example.md"),
        &[
            "--theme=TwoDark".to_string(),
            "--italic-text=always".to_string(),
        ],
    );

    assert_eq!(
        args,
        vec![
            OsString::from("--color=always"),
            OsString::from("--paging=never"),
            OsString::from("--style=plain"),
            OsString::from("--wrap=never"),
            OsString::from("--theme=TwoDark"),
            OsString::from("--italic-text=always"),
            OsString::from("--"),
            OsString::from("/tmp/example.md"),
        ]
    );
}

#[test]
fn bat_command_resolution_prefers_bat_over_batcat() {
    let temp = tempfile::tempdir().unwrap();
    let bat = temp.path().join("bat");
    let batcat = temp.path().join("batcat");
    std::fs::write(&bat, "").unwrap();
    std::fs::write(&batcat, "").unwrap();
    let path = std::env::join_paths([temp.path()]).unwrap();

    assert_eq!(
        resolve_bat_command_from_path(Some(path.as_os_str())),
        Some(bat)
    );
}

#[test]
fn bat_command_resolution_falls_back_to_batcat() {
    let temp = tempfile::tempdir().unwrap();
    let batcat = temp.path().join("batcat");
    std::fs::write(&batcat, "").unwrap();
    let path = std::env::join_paths([temp.path()]).unwrap();

    assert_eq!(
        resolve_bat_command_from_path(Some(path.as_os_str())),
        Some(batcat)
    );
}

#[test]
fn bat_preview_output_parses_ansi_styles() {
    let lines = parse_bat_preview_output(b"\x1b[31mlet\x1b[0m value", 1).unwrap();

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].spans[0].content.as_ref(), "let");
    assert_eq!(lines[0].spans[0].style.fg, Some(Color::Red));
    assert_eq!(lines[0].spans[1].content.as_ref(), " value");
}

#[test]
fn bat_preview_output_rejects_line_count_mismatch() {
    let err = parse_bat_preview_output(b"one\ntwo", 1).unwrap_err();

    assert!(err.contains("line count 2"));
}

#[test]
fn preview_line_overlays_query_match_on_syntax_spans() {
    let syntax = Line::from(vec![
        Span::styled("let", Style::default().fg(Color::Red)),
        Span::styled(" value", Style::default().fg(Color::Blue)),
    ]);
    let line = highlighted_preview_line(
        3,
        "let value",
        Some(&syntax),
        &["let".to_string()],
        true,
        false,
        false,
    );

    assert_eq!(line.spans[0].content.as_ref(), "    3 ");
    assert_eq!(line.spans[1].content.as_ref(), "let");
    assert_eq!(line.spans[1].style.fg, Some(Color::Yellow));
    assert!(line.spans[1].style.add_modifier.contains(Modifier::BOLD));
    assert_eq!(line.spans[2].content.as_ref(), " value");
    assert_eq!(line.spans[2].style.fg, Some(Color::Blue));
}

#[test]
fn preview_line_marks_copy_selection_in_gutter_only() {
    let line = highlighted_preview_line(9, "plain text", None, &[], false, true, true);

    assert_eq!(line.spans[0].style.fg, Some(Color::Green));
    assert!(line.spans[0].style.add_modifier.contains(Modifier::BOLD));
    assert_eq!(line.spans[0].style.bg, None);
    assert_eq!(line.spans[1].content.as_ref(), "plain text");
    assert_eq!(line.spans[1].style.bg, None);
}

#[test]
fn preview_line_marks_cursor_in_gutter_only() {
    let line = highlighted_preview_line(9, "plain text", None, &[], false, true, false);

    assert_eq!(line.spans[0].style.fg, Some(Color::Cyan));
    assert!(line.spans[0].style.add_modifier.contains(Modifier::BOLD));
    assert_eq!(line.spans[0].style.bg, None);
    assert_eq!(line.spans[1].style.bg, None);
}

#[test]
fn enter_in_results_requests_editor_open() {
    let mut app = test_app();
    app.results = vec![search_result("/tmp/example.md", 7)];

    app.handle_key(key(KeyCode::Enter)).unwrap();

    assert_eq!(app.focus, Focus::Results);
    assert_eq!(
        app.pending_editor,
        Some(EditorRequest {
            path: PathBuf::from("/tmp/example.md"),
            line: Some(7),
        })
    );
}

#[test]
fn ctrl_o_opens_selected_result_in_tmux_pane_and_quits() {
    *TMUX_OPEN_CALL.lock().unwrap() = None;
    let mut app = test_app();
    app.config.app.editor.command = "nvim --clean".to_string();
    app.tmux_pane_opener = record_tmux_pane_open;
    app.results = vec![search_result("/tmp/example.md", 7)];

    app.handle_key(ctrl_key('o')).unwrap();

    assert!(app.should_quit);
    assert_eq!(app.pending_editor, None);
    assert_eq!(
        *TMUX_OPEN_CALL.lock().unwrap(),
        Some(TmuxOpenCall {
            editor: "nvim --clean".to_string(),
            path: PathBuf::from("/tmp/example.md"),
            line: Some(7),
        })
    );
}

#[test]
fn ctrl_o_failure_keeps_neith_open() {
    let mut app = test_app();
    app.tmux_pane_opener = fail_tmux_pane_open;
    app.results = vec![search_result("/tmp/example.md", 7)];

    app.handle_key(ctrl_key('o')).unwrap();

    assert!(!app.should_quit);
    assert_eq!(app.pending_editor, None);
    assert_eq!(app.status, "tmux pane open failed: tmux missing");
}

#[test]
fn tab_in_results_focuses_preview() {
    let mut app = test_app();

    app.handle_key(key(KeyCode::Tab)).unwrap();

    assert_eq!(app.focus, Focus::Preview);
    assert_eq!(app.pending_editor, None);
}

#[test]
fn tab_in_preview_focuses_results() {
    let mut app = test_app();
    app.focus = Focus::Preview;

    app.handle_key(key(KeyCode::Tab)).unwrap();

    assert_eq!(app.focus, Focus::Results);
}

#[test]
fn enter_in_preview_still_starts_copy_selection() {
    let mut app = test_app();
    app.focus = Focus::Preview;
    app.preview.lines = vec!["first".to_string(), "second".to_string()];
    app.preview.cursor = 1;

    app.handle_key(key(KeyCode::Enter)).unwrap();

    assert!(app.preview.copy_mode);
    assert_eq!(app.preview.anchor, Some(1));
    assert_eq!(app.pending_editor, None);
}

#[test]
fn ctrl_c_on_multi_block_note_opens_quick_copy_chooser() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("example.md");
    std::fs::write(
        &path,
        "# Example\n\n```bash\nfirst\n```\n\n```bash\nsecond\n```\n",
    )
    .unwrap();
    let mut app = test_app();
    app.results = vec![search_result(path.to_str().unwrap(), 1)];

    app.handle_key(ctrl_key('c')).unwrap();

    assert_eq!(app.focus, Focus::QuickCopy);
    let quick_copy = app.quick_copy.as_ref().unwrap();
    assert_eq!(quick_copy.blocks.len(), 2);
    assert_eq!(quick_copy.selected, 0);

    app.handle_key(key(KeyCode::Down)).unwrap();
    assert_eq!(app.quick_copy.as_ref().unwrap().selected, 1);

    app.handle_key(key(KeyCode::Esc)).unwrap();
    assert_eq!(app.focus, Focus::Results);
    assert_eq!(app.quick_copy, None);
    assert_eq!(app.status, "quick-copy cancelled");
}

#[test]
fn ctrl_c_on_non_note_result_reports_status() {
    let mut app = test_app();
    let mut result = search_result("/tmp/example.txt", 1);
    result.source_kind = "man".to_string();
    app.results = vec![result];

    app.handle_key(ctrl_key('c')).unwrap();

    assert_eq!(app.focus, Focus::Results);
    assert_eq!(app.status, "quick-copy supports markdown notes only");
}

#[test]
fn prompt_omits_all_filter() {
    let mut app = test_app();
    app.mode = MatchMode::Exact;
    app.query = "input query".to_string();

    assert_eq!(prompt_text(&app), "X:all> input query");
}

#[test]
fn prompt_includes_specific_filter() {
    let mut app = test_app();
    app.query = "input query".to_string();
    app.filter = SourceFilter::Names;
    app.library_scopes = vec![LibraryScope::Alias("devdocs".to_string())];
    app.library_index = 0;

    assert_eq!(prompt_text(&app), "F:devdocs:names> input query");
}

#[test]
fn prompt_uses_configured_separator() {
    let mut app = test_app();
    app.query = "input query".to_string();
    app.config.app.ui.prompt.separator = "/".to_string();

    assert_eq!(prompt_text(&app), "F/all> input query");
}

#[test]
fn prompt_uses_configured_right_separator() {
    let mut app = test_app();
    app.query = "input query".to_string();
    app.config.app.ui.prompt.right_separator = "|".to_string();

    assert_eq!(prompt_text(&app), "F:all| input query");
}

#[test]
fn prompt_line_colors_query_white() {
    let mut app = test_app();
    app.query = "input query".to_string();

    let line = prompt_line(&app);
    let query = line.spans.last().unwrap();

    assert_eq!(query.content.as_ref(), "input query");
    assert_eq!(query.style.fg, Some(Color::White));
}

#[test]
fn prompt_line_colors_exact_and_filter_tokens() {
    let mut app = test_app();
    app.mode = MatchMode::Exact;
    app.filter = SourceFilter::Names;
    app.query = "input query".to_string();

    let line = prompt_line(&app);

    assert_eq!(line.spans[0].content.as_ref(), "X");
    assert_eq!(line.spans[0].style.fg, Some(Color::Red));
    assert_eq!(line.spans[4].content.as_ref(), "names");
    assert_eq!(line.spans[4].style.fg, Some(Color::Green));
}

#[test]
fn empty_prompt_separators_fall_back_to_defaults() {
    let mut app = test_app();
    app.config.app.ui.prompt.separator.clear();
    app.config.app.ui.prompt.right_separator.clear();

    assert_eq!(prompt_text(&app), "F:all> ");
}

#[test]
fn ctrl_x_toggles_mode_only() {
    let mut app = test_app();
    app.mode = MatchMode::Fuzzy;
    app.filter = SourceFilter::Names;

    app.handle_key(ctrl_key('x')).unwrap();

    assert_eq!(app.mode, MatchMode::Exact);
    assert_eq!(app.filter, SourceFilter::Names);

    app.handle_key(ctrl_key('x')).unwrap();

    assert_eq!(app.mode, MatchMode::Fuzzy);
    assert_eq!(app.filter, SourceFilter::Names);
}

#[test]
fn ctrl_t_cycles_source_filter() {
    let mut app = test_app();

    app.handle_key(ctrl_key('t')).unwrap();
    assert_eq!(app.filter, SourceFilter::Names);

    app.handle_key(ctrl_key('t')).unwrap();
    assert_eq!(app.filter, SourceFilter::Content);
}

#[test]
fn ctrl_a_enters_add_entry_with_inferred_path() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join("awk")).unwrap();
    let library = Library::new(
        temp.path().to_path_buf(),
        Some("neith-lib".to_string()),
        Some(true),
    );
    let mut app = test_app_with_libraries(vec![library]);
    app.query = "awk print selected fields".to_string();
    app.query_cursor = app.query.len();

    app.handle_key(ctrl_key('a')).unwrap();

    assert_eq!(app.focus, Focus::AddEntry);
    let add_entry = app.add_entry.as_ref().unwrap();
    assert_eq!(add_entry.note_query, "awk print selected fields");
    assert_eq!(
        add_entry.path_text,
        temp.path()
            .join("awk")
            .join("awk-print-selected-fields.md")
            .display()
            .to_string()
    );
    assert_eq!(add_entry.path_cursor, add_entry.path_text.len());
}

#[test]
fn add_entry_confirm_creates_template_note_and_requests_editor() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join(".neith-note-template.md"),
        "# {{TITLE}}\n\nTask: {{QUERY}}\n",
    )
    .unwrap();
    let library = Library::new(
        temp.path().to_path_buf(),
        Some("neith-lib".to_string()),
        Some(true),
    );
    let mut app = test_app_with_libraries(vec![library]);
    app.query = "awk print selected fields".to_string();
    app.query_cursor = app.query.len();

    app.handle_key(ctrl_key('a')).unwrap();
    let path = temp.path().join("custom").join("entry.md");
    let add_entry = app.add_entry.as_mut().unwrap();
    add_entry.path_text = path.display().to_string();
    add_entry.path_cursor = add_entry.path_text.len();
    app.handle_key(key(KeyCode::Enter)).unwrap();

    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "# Entry\n\nTask: awk print selected fields\n"
    );
    assert_eq!(app.pending_editor, Some(EditorRequest { path, line: None }));
    assert_eq!(app.focus, Focus::Results);
    assert_eq!(app.add_entry, None);
}

#[test]
fn ctrl_f_toggles_result_filter_without_changing_query_mode() {
    let mut app = test_app();
    app.mode = MatchMode::Fuzzy;
    app.query = "awk".to_string();
    app.query_cursor = app.query.len();
    app.results = vec![
        search_result_with_title("/tmp/selected.md", 1, "Print Selected Fields"),
        search_result_with_title("/tmp/archive.md", 1, "Archive Logs"),
    ];

    app.handle_key(ctrl_key('f')).unwrap();

    assert_eq!(app.mode, MatchMode::Fuzzy);
    assert_eq!(app.query, "");
    assert!(app.result_refine.is_some());

    app.handle_key(ctrl_key('f')).unwrap();

    assert_eq!(app.query, "awk");
    assert!(app.result_refine.is_none());
    assert_eq!(app.results.len(), 2);
}

#[test]
fn result_filter_fuzzy_filters_current_results() {
    let mut app = test_app();
    app.query = "awk".to_string();
    app.query_cursor = app.query.len();
    app.results = vec![
        search_result_with_title("/tmp/selected.md", 1, "Print Selected Fields"),
        search_result_with_title("/tmp/archive.md", 1, "Archive Logs"),
    ];

    app.handle_key(ctrl_key('f')).unwrap();
    for ch in ['p', 's', 'f'] {
        app.handle_key(key(KeyCode::Char(ch))).unwrap();
    }

    assert_eq!(app.query, "psf");
    assert_eq!(app.results.len(), 1);
    assert_eq!(app.results[0].title, "Print Selected Fields");
    assert_eq!(app.results[0].rank_reason, "result-refine");
}

#[test]
fn preview_scroll_uses_configured_cursor_percent() {
    let mut app = test_app();
    app.preview.lines = (0..100).map(|idx| idx.to_string()).collect();
    app.preview.viewport_height = 20;
    app.preview.cursor = 50;
    app.config.app.ui.preview_cursor_percent = 50;

    app.reposition_preview_scroll();

    assert_eq!(app.preview.scroll, 40);
}

#[test]
fn preview_cursor_percent_clamps_to_viewport() {
    assert_eq!(preview_cursor_offset(20, 0), 0);
    assert_eq!(preview_cursor_offset(20, 50), 10);
    assert_eq!(preview_cursor_offset(20, 100), 19);
    assert_eq!(preview_cursor_offset(20, 200), 19);
}

#[test]
fn mouse_scroll_over_preview_moves_scroll_without_focus_or_cursor_change() {
    let mut app = test_app();
    app.focus = Focus::Results;
    app.preview_area = Some(Rect::new(0, 0, 80, 10));
    app.preview.lines = (0..100).map(|idx| idx.to_string()).collect();
    app.preview.viewport_height = 10;
    app.preview.scroll = 20;
    app.preview.cursor = 50;

    app.handle_mouse(mouse(MouseEventKind::ScrollDown, 5, 5));

    assert_eq!(app.preview.scroll, 23);
    assert_eq!(app.preview.cursor, 50);
    assert_eq!(app.focus, Focus::Results);

    app.handle_mouse(mouse(MouseEventKind::ScrollUp, 5, 5));

    assert_eq!(app.preview.scroll, 20);
    assert_eq!(app.preview.cursor, 50);
    assert_eq!(app.focus, Focus::Results);
}

#[test]
fn mouse_scroll_ignores_events_outside_preview_or_behind_modal() {
    let mut app = test_app();
    app.preview_area = Some(Rect::new(0, 0, 80, 10));
    app.preview.lines = (0..100).map(|idx| idx.to_string()).collect();
    app.preview.viewport_height = 10;
    app.preview.scroll = 20;

    app.handle_mouse(mouse(MouseEventKind::ScrollDown, 5, 12));
    assert_eq!(app.preview.scroll, 20);

    app.focus = Focus::Help;
    app.handle_mouse(mouse(MouseEventKind::ScrollDown, 5, 5));
    assert_eq!(app.preview.scroll, 20);
}

#[test]
fn page_keys_scroll_preview_without_focus_or_cursor_change() {
    let mut app = test_app();
    app.focus = Focus::Results;
    app.selected = 3;
    app.preview.lines = (0..100).map(|idx| idx.to_string()).collect();
    app.preview.viewport_height = 10;
    app.preview.scroll = 20;
    app.preview.cursor = 50;

    app.handle_key(key(KeyCode::PageDown)).unwrap();

    assert_eq!(app.preview.scroll, 30);
    assert_eq!(app.preview.cursor, 50);
    assert_eq!(app.selected, 3);
    assert_eq!(app.focus, Focus::Results);

    app.handle_key(key(KeyCode::PageUp)).unwrap();

    assert_eq!(app.preview.scroll, 20);
    assert_eq!(app.preview.cursor, 50);
    assert_eq!(app.selected, 3);
    assert_eq!(app.focus, Focus::Results);
}

#[test]
fn shift_up_down_scroll_preview_without_moving_selection() {
    let mut app = test_app();
    app.focus = Focus::Preview;
    app.preview.lines = (0..100).map(|idx| idx.to_string()).collect();
    app.preview.viewport_height = 10;
    app.preview.scroll = 20;
    app.preview.cursor = 50;

    app.handle_key(shift_key(KeyCode::Down)).unwrap();

    assert_eq!(app.preview.scroll, 21);
    assert_eq!(app.preview.cursor, 50);
    assert_eq!(app.focus, Focus::Preview);

    app.handle_key(shift_key(KeyCode::Up)).unwrap();

    assert_eq!(app.preview.scroll, 20);
    assert_eq!(app.preview.cursor, 50);
    assert_eq!(app.focus, Focus::Preview);
}

#[test]
fn ctrl_w_deletes_previous_query_word() {
    let mut app = test_app();
    app.query = "alpha beta  ".to_string();
    app.query_cursor = app.query.len();

    app.handle_key(ctrl_key('w')).unwrap();

    assert_eq!(app.query, "alpha ");
    assert_eq!(app.query_cursor, "alpha ".len());
}

#[test]
fn typing_inserts_at_query_cursor() {
    let mut app = test_app();
    app.query = "alpha beta".to_string();
    app.query_cursor = "alpha ".len();

    app.handle_key(key(KeyCode::Char('X'))).unwrap();

    assert_eq!(app.query, "alpha Xbeta");
    assert_eq!(app.query_cursor, "alpha X".len());
}

#[test]
fn j_and_k_type_in_results_mode() {
    let mut app = test_app();

    app.handle_key(key(KeyCode::Char('j'))).unwrap();
    app.handle_key(key(KeyCode::Char('k'))).unwrap();

    assert_eq!(app.query, "jk");
    assert_eq!(app.focus, Focus::Results);
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}
