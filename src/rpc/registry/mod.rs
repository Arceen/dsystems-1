pub mod node;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use crate::rpc::func::execute::execute_rpc_call;
use crate::rpc::func::frame::read::read_rpc_frame;
use crate::rpc::func::frame::write::send_rpc_frame;
use crate::rpc::model::node::{NodeInfo, NodeStatus};
use crate::rpc::model::params::RpcParams;
use crate::rpc::model::request::RpcRequest;
use crate::rpc::model::response::RpcResponse;
use crate::rpc::model::result::RpcResult;
use crate::rpc::model::service::ServiceRegistry;
use crate::rpc::registry::node::{check_node_health, update_node_status};

pub async fn log_registry_status(registry: &Arc<tokio::sync::RwLock<ServiceRegistry>>) {
    let read_registry = registry.read().await;
    let active_count = read_registry.nodes.values()
        .filter(|node| matches!(node.status, NodeStatus::Active))
        .count();
    let total_count = read_registry.nodes.len();

    println!("📊 Registry Status: {active_count}/{total_count} nodes active");
}


// Registry Server for Service Discovery
pub async fn run_registry(port: u16) -> anyhow::Result<()> {
    let registry = Arc::new(tokio::sync::RwLock::new(ServiceRegistry {
        nodes: HashMap::new(),
    }));

    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    println!("🌐 Service Registry running on port {port}");
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
                eprintln!("Registry connection error from {addr}: {e:?}");
            }
        });
    }
}

pub async fn handle_registry_connection(
    mut stream: TcpStream,
    registry: Arc<tokio::sync::RwLock<ServiceRegistry>>,
) -> anyhow::Result<()> {
    let _request_id = 0u64;

    loop {
        // Read RPC request
        let request: RpcRequest = match read_rpc_frame(&mut stream).await {
            Ok(req) => req,
            Err(_) => break, // Client disconnected
        };

        println!("📨 Registry received request: {request:?}");

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
                    println!("  - {node}");
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
                    let address = format!("{ip}:{port}");

                    let mut registry_write = registry.write().await;
                    registry_write.nodes.insert(node_id.clone(), NodeInfo {
                        id: node_id.clone(),
                        address,
                        status: NodeStatus::Active,
                    });

                    println!("📝 Registered node: {} at {}:{}", node_id, ip, port);

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
                println!("value: {value}");
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
