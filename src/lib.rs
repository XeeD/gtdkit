pub mod cli;
mod constants;
mod defaults;
mod docs;
mod email;
mod fs_store;
mod time;

use clap::CommandFactory;
use miette::Result;

pub async fn run(cli: cli::Cli) -> Result<()> {
    match cli.command {
        cli::Commands::Docs { command } => docs::run(command),
        cli::Commands::Email { command } => email::run(command),
        cli::Commands::Completions { shell } => {
            let mut cmd = cli::Cli::command();
            match shell {
                cli::CompletionShell::Zsh => {
                    clap_complete::generate(
                        clap_complete::Shell::Zsh,
                        &mut cmd,
                        "gtdkit",
                        &mut std::io::stdout(),
                    );
                }
            }
            Ok(())
        }
    }
}
