use std::io::{self, Write};
use std::process;
use std::sync::mpsc;
use std::thread;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use neith::action::open_editor;
use neith::cli::{Cli, Command, CompletionShell, ConfigCommand, JsonCommand};
use neith::config::RuntimeConfig;
use neith::diagnostics;
use neith::indexer::{IndexManager, ensure_indexes};
use neith::note;
use neith::query::{LibraryScope, MatchMode, SearchRequest, SourceFilter};
use neith::search::SearchEngine;

fn main() {
    if let Err(err) = run() {
        eprintln!("neith: {err:#}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    if let Some(Command::Completions(args)) = &cli.command {
        print_completions(args.shell)?;
        return Ok(());
    }

    if let Some(Command::Config(args)) = &cli.command {
        match &args.command {
            ConfigCommand::Init { force } => {
                let path =
                    RuntimeConfig::init_config(cli.config.clone(), *force, cli.libs.as_deref())?;
                println!("{}", path.display());
                return Ok(());
            }
        }
    }

    if let Some(Command::Healthcheck(args)) = &cli.command {
        let report = diagnostics::healthcheck(cli.config.clone(), cli.libs.as_deref());
        if args.json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            println!(
                "{}",
                diagnostics::render_healthcheck(&report, diagnostics::use_color_stdout())
            );
        }
        process::exit(report.exit_code());
    }

    let runtime = RuntimeConfig::load(cli.config.clone(), cli.libs.as_deref())?;

    match &cli.command {
        Some(Command::Index(args)) => {
            let rebuild = cli.rebuild || args.rebuild;
            let stats = ensure_indexes(&runtime.libraries, rebuild, |library, stage| {
                eprintln!("{}: {stage}", library.alias);
            })?;
            println!(
                "{}",
                diagnostics::render_index_table(
                    runtime.libraries.iter().zip(stats.iter()),
                    diagnostics::use_color_stdout(),
                )
            );
        }
        Some(Command::Add(args)) => {
            let (path, query) = if args.pick_folder {
                let title = args.query.join(" ");
                note::validate_note_title(&title)?;
                let library = note::target_library(&runtime.libraries)?;
                let Some(folder) = note::pick_note_folder(library)? else {
                    eprintln!("folder selection cancelled");
                    return Ok(());
                };
                (
                    note::infer_note_path_from_parts(library, &folder, &title)?,
                    note::format_note_input(&folder, &title),
                )
            } else {
                let path = note::infer_note_path_from_args(&runtime.libraries, &args.query)?;
                let query = note::format_note_input(&args.query[0], &args.query[1..].join(" "));
                (path, query)
            };
            let edit = note::prepare_note_edit(&path, &query)?;
            let status = open_editor(&runtime.app.editor.command, edit.editor_path(), None)?;
            match edit.finish(status.success())? {
                note::NoteEditOutcome::Created(path) => eprintln!("created {}", path.display()),
                note::NoteEditOutcome::Discarded => eprintln!("new entry cancelled"),
                note::NoteEditOutcome::OpenedExisting => {}
            }
            process::exit(status.code().unwrap_or(0));
        }
        Some(Command::CompleteFolders(args)) => {
            let library = note::target_library(&runtime.libraries)?;
            for folder in note::list_note_folders(library)? {
                if args
                    .prefix
                    .as_ref()
                    .is_none_or(|prefix| folder.starts_with(prefix))
                {
                    println!("{folder}");
                }
            }
        }
        Some(Command::Json(args)) => match &args.command {
            JsonCommand::Query { query, limit } => {
                ensure_indexes(&runtime.libraries, cli.rebuild, |library, stage| {
                    eprintln!("{}: {stage}", library.alias);
                })?;
                let manager = IndexManager::open(&runtime.libraries)?;
                let engine = SearchEngine::new(manager);
                let request = SearchRequest {
                    query: query.join(" "),
                    filter: SourceFilter::All,
                    mode: MatchMode::Fuzzy,
                    library: LibraryScope::All,
                    limit: *limit,
                };
                let results = engine.search(&request);
                println!("{}", serde_json::to_string_pretty(&results)?);
            }
        },
        Some(Command::Status(args)) => {
            let rows = diagnostics::collect_status_rows(&runtime.libraries)?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&rows)?);
            } else {
                println!(
                    "{}",
                    diagnostics::render_status_table(&rows, diagnostics::use_color_stdout())
                );
            }
        }
        Some(Command::Completions(_)) => {}
        Some(Command::Healthcheck(_)) => {}
        Some(Command::Config(_)) => {}
        None => {
            ensure_ready_indexes(&runtime, cli.rebuild)?;
            let manager = IndexManager::open(&runtime.libraries)?;
            let refresh = maybe_refresh_in_background(runtime.libraries.clone());
            let engine = SearchEngine::new(manager);
            neith::tui::run_with_refresh(runtime, engine, cli.query.join(" "), refresh)
                .context("TUI failed")?;
        }
    }

    Ok(())
}

fn print_completions(shell: CompletionShell) -> Result<()> {
    let mut command = Cli::command();
    let name = command.get_name().to_string();
    let mut output = Vec::new();
    match shell {
        CompletionShell::Bash => {
            generate(clap_complete::shells::Bash, &mut command, name, &mut output);
        }
        CompletionShell::Zsh => {
            generate(clap_complete::shells::Zsh, &mut command, name, &mut output);
        }
    }
    let script = String::from_utf8(output)?;
    let script = match shell {
        CompletionShell::Bash => render_bash_completions(script)?,
        CompletionShell::Zsh => render_zsh_completions(script)?,
    };
    io::stdout().write_all(script.as_bytes())?;
    Ok(())
}

const BASH_ADD_COMPLETION: &str = r#"_neith_add_folders() {
    local i add_index=-1
    for ((i=1; i<COMP_CWORD; i++)); do
        if [[ "${COMP_WORDS[i]}" == add ]]; then
            add_index=$i
            break
        fi
    done
    (( add_index >= 0 )) || return 1
    for ((i=add_index+1; i<COMP_CWORD; i++)); do
        case "${COMP_WORDS[i]}" in
            --pick-folder) return 1 ;;
            -h|--help) ;;
            *) return 1 ;;
        esac
    done
    local -a neith_opts=()
    for ((i=1; i<add_index; i++)); do
        case "${COMP_WORDS[i]}" in
            --config|--libs)
                if (( i + 1 < add_index )); then
                    neith_opts+=("${COMP_WORDS[i]}" "${COMP_WORDS[i+1]}")
                    ((i++))
                fi
                ;;
            --config=*|--libs=*) neith_opts+=("${COMP_WORDS[i]}") ;;
        esac
    done
    COMPREPLY=()
    local match
    while IFS= read -r match; do
        COMPREPLY+=("$match")
    done < <("${COMP_WORDS[0]}" "${neith_opts[@]}" __complete-folders "$cur" 2>/dev/null)
    compopt -o filenames 2>/dev/null || true
    return 0
}

"#;

const ZSH_ADD_COMPLETION: &str = r#"_neith_add_folders() {
    local -a neith_opts matches
    local i output
    for ((i=1; i<=$#words; i++)); do
        case "$words[i]" in
            --pick-folder) _default; return ;;
            --config|--libs)
                if (( i < $#words )); then
                    neith_opts+=("$words[i]" "$words[i+1]")
                    ((i++))
                fi
                ;;
            --config=*|--libs=*) neith_opts+=("$words[i]") ;;
        esac
    done
    output=$(command neith "${neith_opts[@]}" __complete-folders "$PREFIX" 2>/dev/null) || return 1
    [[ -n "$output" ]] || return 1
    matches=("${(@f)output}")
    (( $#matches )) && compadd -- "${matches[@]}"
}

"#;

fn render_bash_completions(script: String) -> Result<String> {
    let marker = "        neith__subcmd__add)\n";
    if !script.contains(marker) {
        anyhow::bail!("generated Bash completion has no add command");
    }
    let script = script.replace(marker, &format!("{marker}            if [[ \"${{cur}}\" != -* ]] && _neith_add_folders; then\n                return 0\n            fi\n"));
    let script = script.replace(" add __complete-folders completions", " add completions");
    Ok(format!("{BASH_ADD_COMPLETION}{script}"))
}

fn render_zsh_completions(mut script: String) -> Result<String> {
    let start = script
        .find("(add)\n")
        .context("generated Zsh completion has no add command")?;
    let end = start
        + script[start..]
            .find(";;")
            .context("generated Zsh add completion is incomplete")?;
    let branch = &script[start..end];
    let argument_start = branch
        .find("'*::")
        .context("generated Zsh add completion has no query argument")?;
    let argument_end = argument_start
        + branch[argument_start..]
            .find("_default'")
            .context("generated Zsh add completion has no query action")?
        + "_default'".len();
    let original = &branch[argument_start..argument_end];
    let replaced = branch.replace(
        original,
        "'1:folder:_neith_add_folders' \\\n'*::title:_default'",
    );
    script.replace_range(start..end, &replaced);
    script = script.replace("'__complete-folders:' \\\n", "");
    let rest = script
        .strip_prefix("#compdef neith\n")
        .context("generated Zsh completion has no compdef header")?;
    Ok(format!("#compdef neith\n{ZSH_ADD_COMPLETION}{rest}"))
}

fn ensure_ready_indexes(runtime: &RuntimeConfig, rebuild: bool) -> Result<()> {
    if rebuild || !IndexManager::has_usable_indexes(&runtime.libraries) {
        ensure_indexes(&runtime.libraries, rebuild, |library, stage| {
            eprintln!("{}: {stage}", library.alias);
        })?;
    }
    Ok(())
}

fn maybe_refresh_in_background(
    libraries: Vec<neith::library::Library>,
) -> mpsc::Receiver<neith::tui::RefreshEvent> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let result = ensure_indexes(&libraries, false, |library, stage| {
            let _ = sender.send(neith::tui::RefreshEvent::Progress {
                alias: library.alias.clone(),
                stage: stage.to_string(),
            });
        })
        .map(|_| ());
        let _ = sender.send(neith::tui::RefreshEvent::Finished(result));
    });
    receiver
}
