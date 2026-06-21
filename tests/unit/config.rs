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
    let app: AppConfig = toml::from_str("[clipboard]\ncommand = \"xclip -sel clip\"\n").unwrap();

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
    let sample = include_str!("../../config-sample.toml");
    let app: AppConfig = toml::from_str(sample).unwrap();

    assert_eq!(app.ui.prompt.separator, ":");
    assert_eq!(app.ui.prompt.right_separator, ">");
    assert_eq!(app.ui.prompt.colors.query, "white");
}
