pub mod rpc;
pub  mod cmd;
pub mod app;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, AsyncBufReadExt};
use serde::{Serialize, Deserialize};
use clap::{Parser, Subcommand};
use crate::app::init_app;

#[tokio::main]
async fn main() -> Result<()> {
    init_app();
    Ok(())
}