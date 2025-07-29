use clap::Parser;
use crate::cmd::command::Commands;

#[derive(Parser)]
#[command(name = "distributed-kv")]
#[command(about = "Distributed RPC Key-Value Store")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}
