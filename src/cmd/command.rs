use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    /// Start a registry server for service discovery
    Registry {
        #[arg(long, default_value = "3000")]
        port: u16,
    },
    /// Start a key-value node
    Node {
        /// Node identifier
        #[arg(long)]
        id: String,
        /// Port for this node
        #[arg(long)]
        port: u16,
        /// Registry address for service discovery
        #[arg(long, default_value = "127.0.0.1:3000")]
        registry: String,
    },
    /// Run interactive client
    Client {
        /// Registry address to discover nodes
        #[arg(long, default_value = "127.0.0.1:3000")]
        registry: String,
    },
}