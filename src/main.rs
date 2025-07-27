use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, AsyncBufReadExt};
use serde::{Serialize, Deserialize};
use clap::{Parser, Subcommand};
use rand::prelude::IteratorRandom;
use rand::seq::SliceRandom;

// RPC Protocol Definition
#[derive(Serialize, Deserialize, Debug, Clone)]
struct RpcRequest {
    id: u64,
    method: String,
    params: RpcParams,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct RpcResponse {
    id: u64,
    result: Option<RpcResult>,
    error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "method", content = "args")]
enum RpcParams {
    Get { key: String },
    Put { key: String, value: String },
    ListKeys,
    Ping { message: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
enum RpcResult {
    GetResult { value: Option<String> },
    PutResult { success: bool },
    ListResult { keys: Vec<String> },
    PingResult { response: String },
}

// Service Discovery
#[derive(Serialize, Deserialize, Debug, Clone)]
struct NodeInfo {
    id: String,
    address: String,
    status: NodeStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum NodeStatus {
    Active,
    Inactive,
}

#[derive(Serialize, Deserialize, Debug)]
struct ServiceRegistry {
    nodes: HashMap<String, NodeInfo>,
}

// Configuration
#[derive(Serialize, Deserialize, Debug)]
struct Config {
    nodes: HashMap<String, String>,
    discovery: DiscoveryConfig,
}

#[derive(Serialize, Deserialize, Debug)]
struct DiscoveryConfig {
    registry_port: u16,
}

#[derive(Parser)]
#[command(name = "distributed-kv")]
#[command(about = "Distributed RPC Key-Value Store")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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

#[tokio::main]
async fn main() -> Result<()> {
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


async fn check_node_health(address: &str) -> bool {
    match tokio::time::timeout(
        tokio::time::Duration::from_secs(2), // 2 second timeout
        TcpStream::connect(address)
    ).await {
        Ok(Ok(_stream)) => {
            println!("✅ Node {} is healthy", address);
            true
        },
        Ok(Err(e)) => {
            println!("❌ Node {} failed: {}", address, e);
            false
        },
        Err(_) => {
            println!("⏱️ Node {} timed out", address);
            false
        }
    }
}

async fn update_node_status(registry: Arc<tokio::sync::RwLock<ServiceRegistry>>, node_id: String, is_healthy: bool) {
    let mut write_registry = registry.write().await;

    if let Some(node) = write_registry.nodes.get_mut(&node_id) {
        let new_status = if is_healthy {
            NodeStatus::Active
        } else {
            NodeStatus::Inactive
        };

        // Only log if status changed
        if matches!((&node.status, &new_status),
                   (NodeStatus::Active, NodeStatus::Inactive) |
                   (NodeStatus::Inactive, NodeStatus::Active)) {
            println!("🔄 Node {} status changed: {:?} -> {:?}",
                     node_id, node.status, new_status);
        }

        node.status = new_status;
    }
}

async fn log_registry_status(registry: &Arc<tokio::sync::RwLock<ServiceRegistry>>) {
    let read_registry = registry.read().await;
    let active_count = read_registry.nodes.values()
        .filter(|node| matches!(node.status, NodeStatus::Active))
        .count();
    let total_count = read_registry.nodes.len();

    println!("📊 Registry Status: {}/{} nodes active", active_count, total_count);
}


// Registry Server for Service Discovery
async fn run_registry(port: u16) -> Result<()> {
    let registry = Arc::new(tokio::sync::RwLock::new(ServiceRegistry {
        nodes: HashMap::new(),
    }));

    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    println!("🌐 Service Registry running on port {}", port);
    let cloned_registry = registry.clone();
    // Health check task
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3));

        loop {
            interval.tick().await;

            // Get snapshot of current nodes
            let nodes_to_check: Vec<(String, String)> = {
                let read_registry = cloned_registry.read().await;
                read_registry.nodes.iter()
                    .map(|(id, node)| (id.clone(), node.address.clone()))
                    .collect()
            };

            // Check each node's health concurrently
            let health_check_futures = nodes_to_check.into_iter().map(|(node_id, address)| {
                let registry_clone = cloned_registry.clone();
                async move {
                    let is_healthy = check_node_health(&address).await;
                    update_node_status(registry_clone, node_id, is_healthy).await;
                }
            });

            // Wait for all health checks to complete
            futures::future::join_all(health_check_futures).await;

            // Log current status
            log_registry_status(&cloned_registry).await;
        }
    });


    loop {
        let (stream, addr) = listener.accept().await?;
        let registry_clone = registry.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_registry_connection(stream, registry_clone).await {
                eprintln!("Registry connection error from {}: {:?}", addr, e);
            }
        });
    }
}

async fn handle_registry_connection(
    mut stream: TcpStream,
    registry: Arc<tokio::sync::RwLock<ServiceRegistry>>,
) -> Result<()> {
    let _request_id = 0u64;

    loop {
        // Read RPC request
        let request: RpcRequest = match read_rpc_frame(&mut stream).await {
            Ok(req) => req,
            Err(_) => break, // Client disconnected
        };

        println!("📨 Registry received request: {:?}", request);

        let response = match &request.params {
            RpcParams::Get { key } if key == "nodes" => {
                // Special case: get list of active nodes
                let registry_read = registry.read().await;
                let active_nodes: Vec<String> = registry_read.nodes
                    .values()
                    .filter(|node| matches!(node.status, NodeStatus::Active))
                    .map(|node| format!("{}:{}", node.id, node.address))
                    .collect();

                println!("🔍 Node discovery request - found {} active nodes", active_nodes.len());
                for node in &active_nodes {
                    println!("  - {}", node);
                }

                println!("Sending discover reponse: {:#?}",
                         RpcResponse {
                             id: request.id,
                             result: Some(RpcResult::ListResult { keys: active_nodes.clone() }),
                             error: None,
                         });
                RpcResponse {
                    id: request.id,
                    result: Some(RpcResult::ListResult { keys: active_nodes }),
                    error: None,
                }
            }
            RpcParams::Put { key, value } if key == "register" => {
                // Register a new node
                // Expected format: "node_id:ip:port"
                let parts: Vec<&str> = value.split(':').collect();
                if parts.len() == 3 {
                    let node_id = parts[0].to_string();
                    let ip = parts[1].to_string();
                    let port = parts[2].to_string();
                    let address = format!("{}:{}", ip, port);

                    let mut registry_write = registry.write().await;
                    registry_write.nodes.insert(node_id.clone(), NodeInfo {
                        id: node_id.clone(),
                        address,
                        status: NodeStatus::Active,
                    });

                    println!("📝 Registered node: {} at {}", node_id, format!("{}:{}", ip, port));

                    RpcResponse {
                        id: request.id,
                        result: Some(RpcResult::PutResult { success: true }),
                        error: None,
                    }
                } else {
                    RpcResponse {
                        id: request.id,
                        result: None,
                        error: Some(format!("Invalid registration format. Expected 'node_id:ip:port', got '{}' with {} parts", value, parts.len())),
                    }
                }
            }
            RpcParams::Put { key, value } if key != "register" => {
                let read_registry = registry.read().await;
                println!("read registry");
                // Collect all node addresses
                let node_addrs: Vec<String> = read_registry.nodes.values()
                    .map(|node| node.address.clone()).collect();
                println!("node address: {node_addrs:#?}");
                if node_addrs.is_empty() {
                    RpcResponse {
                        id: request.id,
                        result: None,
                        error: Some("No nodes available".to_owned())
                    }
                } else {
                    // Spawn a task for each node and collect JoinHandles
                    node_addrs.into_iter()
                        .for_each(|node_addr| {
                            let request = request.clone();
                            tokio::spawn(async move {
                                execute_rpc_call(&node_addr, request).await
                            });
                        });
                    // If we get here, all tasks failed
                    RpcResponse {
                        id: request.id,
                        result: None,
                        error: None,
                    }
                }
            }
            _ => {
                println!("❓ Unknown registry operation: {:?}", request.params);
                RpcResponse {
                    id: request.id,
                    result: None,
                    error: Some("Unknown registry operation".to_string()),
                }
            }
        };

        send_rpc_frame(&mut stream, &response).await?;
    }

    Ok(())
}


// Key-Value Node
async fn run_node(id: String, port: u16, registry_addr: String) -> Result<()> {
    let storage = Arc::new(tokio::sync::RwLock::new(HashMap::<String, String>::new()));

    // Register with service discovery
    register_with_registry(&id, port, &registry_addr).await?;

    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    println!("🚀 Node '{}' running on port {}", id, port);

    loop {
        let (stream, addr) = listener.accept().await?;
        let storage_clone = storage.clone();
        let node_id = id.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_node_connection(stream, storage_clone, node_id).await {
                eprintln!("Node connection error from {}: {:?}", addr, e);
            }
        });
    }
}

async fn register_with_registry(node_id: &str, port: u16, registry_addr: &str) -> Result<()> {
    println!("🔗 Connecting to registry at: {}", registry_addr);
    let mut stream = TcpStream::connect(registry_addr).await
        .map_err(|e| anyhow::anyhow!("Failed to connect to registry {}: {}", registry_addr, e))?;

    let request = RpcRequest {
        id: 1,
        method: "put".to_string(),
        params: RpcParams::Put {
            key: "register".to_string(),
            value: format!("{}:127.0.0.1:{}", node_id, port),
        },
    };

    println!("📤 Sending registration: {}", format!("{}:127.0.0.1:{}", node_id, port));
    send_rpc_frame(&mut stream, &request).await?;
    let response: RpcResponse = read_rpc_frame(&mut stream).await?;

    println!("📨 Registry response: {:?}", response);

    if response.error.is_some() {
        anyhow::bail!("Registration failed: {:?}", response.error);
    }

    println!("✅ Registered with service registry");
    Ok(())
}

async fn handle_node_connection(
    mut stream: TcpStream,
    storage: Arc<tokio::sync::RwLock<HashMap<String, String>>>,
    node_id: String,
) -> Result<()> {
    loop {
        let request: RpcRequest = match read_rpc_frame(&mut stream).await {
            Ok(req) => req,
            Err(_) => break,
        };

        println!("📨 Node '{}' received RPC: {:?}", node_id, request.params);

        let response = match request.params {
            RpcParams::Get { key } => {
                let storage_read = storage.read().await;
                let value = storage_read.get(&key).cloned();
                RpcResponse {
                    id: request.id,
                    result: Some(RpcResult::GetResult { value }),
                    error: None,
                }
            }
            RpcParams::Put { key, value } => {
                let mut storage_write = storage.write().await;
                storage_write.insert(key, value);
                RpcResponse {
                    id: request.id,
                    result: Some(RpcResult::PutResult { success: true }),
                    error: None,
                }
            }
            RpcParams::ListKeys => {
                let storage_read = storage.read().await;
                let keys: Vec<String> = storage_read.keys().cloned().collect();
                RpcResponse {
                    id: request.id,
                    result: Some(RpcResult::ListResult { keys }),
                    error: None,
                }
            }
            RpcParams::Ping { message } => {
                RpcResponse {
                    id: request.id,
                    result: Some(RpcResult::PingResult {
                        response: format!("pong from {}: {}", node_id, message)
                    }),
                    error: None,
                }
            }
        };

        send_rpc_frame(&mut stream, &response).await?;
    }

    Ok(())
}

// Interactive Client
async fn run_client(registry_addr: String) -> Result<()> {
    println!("🔍 Discovering nodes from registry...");
    let mut nodes = discover_nodes(&registry_addr).await?;

    if nodes.is_empty() {
        println!("❌ No active nodes found!");
        return Ok(());
    }

    println!("✅ Found {} active nodes:", nodes.len());
    for node in &nodes {
        println!("  - {}", node);
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
                println!("  - {}", node);
            }
            nodes = updated_nodes;
            continue;
        }

        let request = match parse_client_command(trimmed, request_id) {
            Ok(req) => req,
            Err(e) => {
                println!("❌ {}", e);
                continue;
            }
        };
        let method = request.method.clone();
        match method.as_str() {
            "put" => {
                // Hit registry for put request
                match execute_rpc_call(&registry_addr, request).await {
                    Ok(response) => display_response(response),
                    Err(e) => println!("❌ RPC call failed: {:?}", e),
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
                    Err(e) => println!("❌ RPC call failed: {:?}", e),
                }
            }

        }



        request_id += 1;
    }

    Ok(())
}

async fn discover_nodes(registry_addr: &str) -> Result<Vec<String>> {
    println!("🔍 Connecting to registry at: {}", registry_addr);
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

    println!("📨 Registry response: {:?}", response);

    match response.result {
        Some(RpcResult::ListResult { keys }) => {
            println!("✅ Found {} nodes from registry", keys.len());
            Ok(keys)
        }
        Some(other) => {
            println!("❌ Unexpected response type: {:?}", other);
            Ok(vec![])
        }
        None => {
            println!("❌ Registry returned error: {:?}", response.error);
            Ok(vec![])
        }
    }
}

fn parse_client_command(input: &str, id: u64) -> Result<RpcRequest> {
    let parts: Vec<&str> = input.split_whitespace().collect();

    let params = match parts.as_slice() {
        ["get", key] => RpcParams::Get { key: key.to_string() },
        ["put", key, value @ ..] => {
            if value.is_empty() {
                anyhow::bail!("PUT requires a value");
            }
            RpcParams::Put {
                key: key.to_string(),
                value: value.join(" ")
            }
        },
        ["list"] => RpcParams::ListKeys,
        ["ping", message @ ..] => RpcParams::Ping {
            message: message.join(" ")
        },
        _ => anyhow::bail!("Unknown command"),
    };

    Ok(RpcRequest {
        id,
        method: match &params {
            RpcParams::Get { .. } => "get",
            RpcParams::Put { .. } => "put",
            RpcParams::ListKeys => "list",
            RpcParams::Ping { .. } => "ping",
        }.to_string(),
        params,
    })
}

async fn execute_rpc_call(node_addr: &str, request: RpcRequest) -> Result<RpcResponse> {
    let mut stream = TcpStream::connect(node_addr).await?;
    send_rpc_frame(&mut stream, &request).await?;
    read_rpc_frame(&mut stream).await
}

fn display_response(response: RpcResponse) {
    match response.result {
        Some(RpcResult::GetResult { value }) => {
            match value {
                Some(v) => println!("✅ Found: {}", v),
                None => println!("❌ Key not found"),
            }
        }
        Some(RpcResult::PutResult { success }) => {
            if success {
                println!("✅ Value stored successfully");
            } else {
                println!("❌ Failed to store value");
            }
        }
        Some(RpcResult::ListResult { keys }) => {
            println!("📋 Keys ({}):", keys.len());
            for key in keys {
                println!("  - {}", key);
            }
        }
        Some(RpcResult::PingResult { response }) => {
            println!("🏓 {}", response);
        }
        None => {
            if let Some(error) = response.error {
                println!("❌ Error: {}", error);
            }
        }
    }
}

// RPC Protocol Implementation
async fn read_rpc_frame<T: for<'de> Deserialize<'de>>(stream: &mut TcpStream) -> Result<T> {
    let mut len_bytes = [0u8; 4];
    stream.read_exact(&mut len_bytes).await
        .map_err(|e| anyhow::anyhow!("Failed to read frame length: {}", e))?;
    let len = u32::from_be_bytes(len_bytes) as usize;

    if len > 1024 * 1024 {
        anyhow::bail!("Frame too large: {} bytes", len);
    }

    let mut data = vec![0u8; len];
    stream.read_exact(&mut data).await
        .map_err(|e| anyhow::anyhow!("Failed to read frame data ({} bytes): {}", len, e))?;

    let json_str = std::str::from_utf8(&data)
        .map_err(|e| anyhow::anyhow!("Frame data is not valid UTF-8: {}", e))?;

    println!("client Got this: {json_str}");
    let obj: T = serde_json::from_str(json_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse JSON: {} | Data: {}", e, json_str))?;

    Ok(obj)
}

async fn send_rpc_frame<T: Serialize>(stream: &mut TcpStream, obj: &T) -> Result<()> {
    let json_str = serde_json::to_string(obj)
        .map_err(|e| anyhow::anyhow!("Failed to serialize to JSON: {}", e))?;

    println!("Serialized version: {json_str}");
    let json_bytes = json_str.as_bytes();
    let len = json_bytes.len() as u32;

    stream.write_all(&len.to_be_bytes()).await
        .map_err(|e| anyhow::anyhow!("Failed to write frame length: {}", e))?;
    stream.write_all(json_bytes).await
        .map_err(|e| anyhow::anyhow!("Failed to write frame data: {}", e))?;

    Ok(())
}