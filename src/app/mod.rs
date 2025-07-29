use clap::Parser;
use crate::cmd::cli::Cli;
use crate::cmd::command::Commands;
use crate::rpc::client::run_client;
use crate::rpc::node::run_node;
use crate::rpc::registry::run_registry;

pub async fn init_app() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Registry { port } => {
            run_registry(port).await
        }
        Commands::Node { id, port, registry } => {
            run_node(id, port, registry).await
        }
        Commands::Client { registry } => {
            run_client(registry).await
        }
    }
}