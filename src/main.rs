pub mod rpc;
pub  mod cmd;
pub mod app;

use anyhow::Result;
use crate::app::init_app;

#[tokio::main]
async fn main() -> Result<()> {
    init_app().await
}