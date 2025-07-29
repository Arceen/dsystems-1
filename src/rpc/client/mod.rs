use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpStream;
use crate::rpc::func::display::display_response;
use crate::rpc::func::execute::execute_rpc_call;
use crate::rpc::func::parse_cmd::parse_client_command;

use rand::seq::SliceRandom;
use crate::rpc::func::frame::read::read_rpc_frame;
use crate::rpc::func::frame::write::send_rpc_frame;
use crate::rpc::model::params::RpcParams;
use crate::rpc::model::request::RpcRequest;
use crate::rpc::model::response::RpcResponse;
use crate::rpc::model::result::RpcResult;

// Interactive Client
pub async fn run_client(registry_addr: String) -> anyhow::Result<()> {
    println!("🔍 Discovering nodes from registry...");
    let mut nodes = discover_nodes(&registry_addr).await?;

    if nodes.is_empty() {
        println!("❌ No active nodes found!");
        return Ok(());
    }

    println!("✅ Found {} active nodes:", nodes.len());
    for node in &nodes {
        println!("  - {node}");
    }

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();
    let mut request_id = 1u64;

    println!("\n📖 Commands:");
    println!("  get <key>           - Get value from random node");
    println!("  put <key> <value>   - Set value on random node");
    println!("  list                - List all keys from random node");
    println!("  ping <message>      - Ping random node");
    println!("  nodes               - Show available nodes");
    println!("  q                   - Quit");

    loop {
        print!("💬 > ");
        use std::io::Write;
        std::io::stdout().flush().unwrap();

        line.clear();
        reader.read_line(&mut line).await?;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        if trimmed == "q" || trimmed == "quit" {
            println!("👋 Goodbye!");
            break;
        }

        if trimmed == "nodes" {
            let updated_nodes = discover_nodes(&registry_addr).await?;
            println!("Active nodes:");
            for node in &updated_nodes {
                println!("  - {node}");
            }
            nodes = updated_nodes;
            continue;
        }

        let request = match parse_client_command(trimmed, request_id) {
            Ok(req) => req,
            Err(e) => {
                println!("❌ {e}");
                continue;
            }
        };
        let method = request.method.clone();
        match method.as_str() {
            "put" => {
                // Hit registry for put request
                match execute_rpc_call(&registry_addr, request).await {
                    Ok(response) => display_response(response),
                    Err(e) => println!("❌ RPC call failed: {e:?}"),
                }
            }
            _ => {
                // Pick a random node
                let node_addr = if let Some(addr) = nodes.choose(&mut rand::thread_rng()) {
                    addr.split(':').skip(1).collect::<Vec<_>>().join(":")
                } else {
                    println!("❌ No nodes available");
                    continue;
                };

                match execute_rpc_call(&node_addr, request).await {
                    Ok(response) => display_response(response),
                    Err(e) => println!("❌ RPC call failed: {e:?}"),
                }
            }

        }



        request_id += 1;
    }

    Ok(())
}

pub async fn discover_nodes(registry_addr: &str) -> anyhow::Result<Vec<String>> {
    println!("🔍 Connecting to registry at: {registry_addr}");
    let mut stream = TcpStream::connect(registry_addr).await
        .map_err(|e| anyhow::anyhow!("Failed to connect to registry {}: {}", registry_addr, e))?;

    let request = RpcRequest {
        id: 1,
        method: "get".to_string(),
        params: RpcParams::Get { key: "nodes".to_string() },
    };

    println!("📤 Sending discovery request to registry");
    send_rpc_frame(&mut stream, &request).await?;

    println!("📥 Waiting for registry response...");
    let response: RpcResponse = read_rpc_frame(&mut stream).await?;

    println!("📨 Registry response: {response:?}");

    match response.result {
        Some(RpcResult::ListResult { keys }) => {
            println!("✅ Found {} nodes from registry", keys.len());
            Ok(keys)
        }
        Some(other) => {
            println!("❌ Unexpected response type: {other:?}");
            Ok(vec![])
        }
        None => {
            println!("❌ Registry returned error: {:?}", response.error);
            Ok(vec![])
        }
    }
}

