use crate::error::CliError;
use crate::Cli;
use clap::CommandFactory;
use clap_complete::{generate, Shell};

pub fn execute(shell: Shell) -> Result<(), CliError> {
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "clickup", &mut std::io::stdout());
    Ok(())
}
