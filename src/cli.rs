use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "neith")]
#[command(about = "Native indexed knowledge-library search")]
pub struct Cli {
    #[arg(long, value_name = "PATHS")]
    pub libs: Option<String>,

    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    #[arg(long)]
    pub rebuild: bool,

    #[command(subcommand)]
    pub command: Option<Command>,

    #[arg(trailing_var_arg = true)]
    pub query: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Index(IndexArgs),
    Add(AddArgs),
    #[command(name = "__complete-folders", hide = true)]
    CompleteFolders(CompleteFoldersArgs),
    Completions(CompletionsArgs),
    Config(ConfigArgs),
    Healthcheck(HealthcheckArgs),
    Status(StatusArgs),
    Json(JsonArgs),
}

#[derive(Debug, Args)]
pub struct IndexArgs {
    #[arg(long)]
    pub rebuild: bool,
}

#[derive(Debug, Args)]
pub struct HealthcheckArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct StatusArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct AddArgs {
    #[arg(
        long,
        help = "Choose an existing folder with fzf; remaining words are the title"
    )]
    pub pick_folder: bool,
    #[arg(
        value_name = "WORDS",
        help = "Folder followed by title, or title with --pick-folder"
    )]
    pub query: Vec<String>,
}

#[derive(Debug, Args)]
pub struct CompleteFoldersArgs {
    pub prefix: Option<String>,
}

#[derive(Debug, Args)]
pub struct CompletionsArgs {
    #[arg(value_enum)]
    pub shell: CompletionShell,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
}

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    Init {
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Args)]
pub struct JsonArgs {
    #[command(subcommand)]
    pub command: JsonCommand,
}

#[derive(Debug, Subcommand)]
pub enum JsonCommand {
    Query {
        query: Vec<String>,

        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
}

#[cfg(test)]
#[path = "../tests/unit/cli.rs"]
mod tests;
