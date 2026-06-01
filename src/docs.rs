use clap::{ArgAction, Command, CommandFactory};
use miette::Result;

use crate::cli::{Cli, DocsCommands, DocsFormat};

pub(crate) fn run(command: DocsCommands) -> Result<()> {
    match command {
        DocsCommands::CliReference {
            format: DocsFormat::Markdown,
        } => {
            anstream::print!("{}", cli_reference_markdown());
            Ok(())
        }
    }
}

/// Generate the checked-in CLI reference from Clap's command graph.
///
/// This is the single source of truth for command/flag documentation. It walks
/// `Cli::command()` directly instead of parsing terminal help text, which keeps
/// generation stable and avoids coupling tests to help formatting.
pub fn cli_reference_markdown() -> String {
    let mut output = String::from("# gtdkit CLI Reference\n\n");
    output.push_str("Generated with `gtdkit docs cli-reference --format markdown`.\n\n");
    let command = Cli::command();
    render_command(&mut output, &command, "gtdkit", 2);
    output
}

/// Render one command and all subcommands as Markdown sections.
fn render_command(output: &mut String, command: &Command, path: &str, level: usize) {
    output.push_str(&format!("{} `{}`\n\n", "#".repeat(level), path));
    if let Some(about) = command.get_about().or_else(|| command.get_long_about()) {
        output.push_str(&format!("{}\n\n", about));
    }

    let args: Vec<_> = command
        .get_arguments()
        .filter(|arg| !arg.is_hide_set())
        .collect();
    if !args.is_empty() {
        output.push_str("| Argument | Required | Repeatable | Help |\n");
        output.push_str("| --- | --- | --- | --- |\n");
        for arg in args {
            let names = argument_names(arg);
            let help = arg
                .get_help()
                .or_else(|| arg.get_long_help())
                .map(|help| escape_table(&help.to_string()))
                .unwrap_or_default();
            output.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                names,
                arg.is_required_set(),
                is_repeatable(arg),
                help
            ));
        }
        output.push('\n');
    }

    let subcommands: Vec<_> = command
        .get_subcommands()
        .filter(|subcommand| !subcommand.is_hide_set())
        .collect();
    if !subcommands.is_empty() {
        output.push_str("Subcommands:\n\n");
        for subcommand in &subcommands {
            output.push_str(&format!("- `{}`\n", subcommand.get_name()));
        }
        output.push('\n');
    }
    for subcommand in subcommands {
        render_command(
            output,
            subcommand,
            &format!("{path} {}", subcommand.get_name()),
            level + 1,
        );
    }
}

fn argument_names(arg: &clap::Arg) -> String {
    let mut names = vec![];
    if let Some(short) = arg.get_short() {
        names.push(format!("-{short}"));
    }
    if let Some(long) = arg.get_long() {
        names.push(format!("--{long}"));
    }
    if names.is_empty() {
        names.push(format!("<{}>", arg.get_id()));
    }
    names.join(", ")
}

fn escape_table(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn is_repeatable(arg: &clap::Arg) -> bool {
    matches!(arg.get_action(), ArgAction::Append | ArgAction::Count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_reference_mentions_workflow_commands() {
        let reference = cli_reference_markdown();
        assert!(reference.contains("`gtdkit docs cli-reference`"));
        assert!(reference.contains("`gtdkit email research digest`"));
        assert!(reference.contains("`gtdkit email step dashboard`"));
        assert!(reference.contains("`gtdkit email action complete`"));
    }

    #[test]
    fn checked_in_cli_reference_is_current() {
        let expected = std::fs::read_to_string("docs/cli-reference.md").unwrap();
        assert_eq!(expected.replace("\r\n", "\n"), cli_reference_markdown());
    }
}
