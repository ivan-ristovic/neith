use super::*;
use clap::Parser;

#[test]
fn parses_bash_completions_command() {
    let cli = Cli::try_parse_from(["neith", "completions", "bash"]).unwrap();

    assert!(matches!(
        cli.command,
        Some(Command::Completions(CompletionsArgs {
            shell: CompletionShell::Bash
        }))
    ));
}

#[test]
fn parses_zsh_completions_command() {
    let cli = Cli::try_parse_from(["neith", "completions", "zsh"]).unwrap();

    assert!(matches!(
        cli.command,
        Some(Command::Completions(CompletionsArgs {
            shell: CompletionShell::Zsh
        }))
    ));
}
