use clap::Subcommand;
use crate::error::CliError;

#[derive(Subcommand)]
pub enum McpCommands {
    /// Start the MCP server (reads JSON-RPC from stdin, writes to stdout)
    Serve,
}

pub async fn execute(command: McpCommands) -> Result<(), CliError> {
    match command {
        McpCommands::Serve => {
            let filter = crate::mcp::filter::Filter::resolve(
                crate::mcp::filter::RawFilter::default(),
            )
            .map_err(|e| CliError::ConfigError(e.to_string()))?;
            crate::mcp::serve(filter)
                .await
                .map_err(|e| CliError::ConfigError(e.to_string()))
        }
    }
}
