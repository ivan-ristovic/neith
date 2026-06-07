use std::process;
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
        print_completions(args.shell);
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
            let query = args.query.join(" ");
            let path = note::infer_note_path(&runtime.libraries, &query)?;
            note::create_note(&path, &query)?;
            let status = open_editor(&runtime.app.editor.command, &path, None)?;
            process::exit(status.code().unwrap_or(0));
        }
        Some(Command::Json(args)) => match &args.command {
            JsonCommand::Query { query, limit } => {
                ensure_ready_indexes(&runtime, cli.rebuild)?;
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
            maybe_refresh_in_background(runtime.libraries.clone());
            let engine = SearchEngine::new(manager);
            neith::tui::run(runtime, engine, cli.query.join(" ")).context("TUI failed")?;
        }
    }

    Ok(())
}

fn print_completions(shell: CompletionShell) {
    let mut command = Cli::command();
    let name = command.get_name().to_string();
    match shell {
        CompletionShell::Bash => {
            generate(
                clap_complete::shells::Bash,
                &mut command,
                name,
                &mut std::io::stdout(),
            );
        }
        CompletionShell::Zsh => {
            generate(
                clap_complete::shells::Zsh,
                &mut command,
                name,
                &mut std::io::stdout(),
            );
        }
    }
}

fn ensure_ready_indexes(runtime: &RuntimeConfig, rebuild: bool) -> Result<()> {
    if rebuild || !IndexManager::has_usable_indexes(&runtime.libraries) {
        ensure_indexes(&runtime.libraries, rebuild, |library, stage| {
            eprintln!("{}: {stage}", library.alias);
        })?;
    }
    Ok(())
}

fn maybe_refresh_in_background(libraries: Vec<neith::library::Library>) {
    thread::spawn(move || {
        let _ = ensure_indexes(&libraries, false, |_library, _stage| {});
    });
}
