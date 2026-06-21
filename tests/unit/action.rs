use super::*;

#[test]
fn shell_quote_handles_spaces_and_single_quotes() {
    assert_eq!(shell_quote("plain"), "'plain'");
    assert_eq!(shell_quote("two words"), "'two words'");
    assert_eq!(shell_quote("don't"), "'don'\\''t'");
}

#[test]
fn editor_shell_command_adds_line_path_and_persistent_shell() {
    let command = editor_shell_command(
        "nvim --clean",
        Path::new("/tmp/notes/example file.md"),
        Some(17),
    );

    assert_eq!(
        command,
        "'nvim' '--clean' '+17' '/tmp/notes/example file.md'; exec \"${SHELL:-sh}\""
    );
}

#[test]
fn editor_shell_command_uses_vi_default_and_omits_zero_line() {
    let command = editor_shell_command("", Path::new("/tmp/example.md"), Some(0));

    assert_eq!(command, "'vi' '/tmp/example.md'; exec \"${SHELL:-sh}\"");
}

#[test]
fn split_flag_uses_vertical_for_wide_panes() {
    assert_eq!(split_flag_for_size(120, 40), "-v");
}

#[test]
fn split_flag_uses_horizontal_for_tall_or_square_panes() {
    assert_eq!(split_flag_for_size(40, 120), "-h");
    assert_eq!(split_flag_for_size(80, 80), "-h");
}

#[test]
fn parses_tmux_pane_size() {
    assert_eq!(parse_tmux_pane_size("120 40\n").unwrap(), (120, 40));
}

#[test]
fn pane_size_parse_rejects_invalid_output() {
    let err = parse_tmux_pane_size("120\n").unwrap_err();

    assert!(err.to_string().contains("height is missing"));
}

#[test]
fn copy_with_command_splits_program_and_args() {
    let err = copy_with_command("definitely-missing-neith-copy --flag", "text").unwrap_err();

    assert!(
        err.to_string()
            .contains("failed to start definitely-missing-neith-copy")
    );
}

#[test]
fn copy_with_empty_command_errors() {
    let err = copy_with_command(" ", "text").unwrap_err();

    assert_eq!(err.to_string(), "clipboard command is empty");
}
