use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use crate::rpc::func::frame::read::read_rpc_frame;
use crate::rpc::func::frame::write::send_rpc_frame;
use crate::rpc::model::params::RpcParams;
use crate::rpc::model::request::RpcRequest;
use crate::rpc::model::response::RpcResponse;
use crate::rpc::model::result::RpcResult;

// Key-Value Node
pub async fn run_node(id: String, port: u16, registry_addr: String) -> anyhow::Result<()> {
    let storage = Arc::new(tokio::sync::RwLock::new(HashMap::<String, String>::new()));

    // Register with service discovery
    register_with_registry(&id, port, &registry_addr).await?;

    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    println!("🚀 Node '{id}' running on port {port}");

    loop {
        let (stream, addr) = listener.accept().await?;
        let storage_clone = storage.clone();
        let node_id = id.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_node_connection(stream, storage_clone, node_id).await {
                eprintln!("Node connection error from {addr}: {e:?}");
            }
        });
    }
}

async fn register_with_registry(node_id: &str, port: u16, registry_addr: &str) -> anyhow::Result<()> {
    println!("🔗 Connecting to registry at: {registry_addr}");
    let mut stream = TcpStream::connect(registry_addr).await
        .map_err(|e| anyhow::anyhow!("Failed to connect to registry {}: {}", registry_addr, e))?;

    let request = RpcRequest {
        id: 1,
        method: "put".to_string(),
        params: RpcParams::Put {
            key: "register".to_string(),
            value: format!("{node_id}:127.0.0.1:{port}"),
        },
    };

    println!("📤 Sending registration: {}", format!("{}:127.0.0.1:{}", node_id, port));
    send_rpc_frame(&mut stream, &request).await?;
    let response: RpcResponse = read_rpc_frame(&mut stream).await?;

    println!("📨 Registry response: {response:?}");

    if response.error.is_some() {
        anyhow::bail!("Registration failed: {:?}", response.error);
    }

    println!("✅ Registered with service registry");
    Ok(())
}


pub async fn handle_node_connection(
    mut stream: TcpStream,
    storage: Arc<tokio::sync::RwLock<HashMap<String, String>>>,
    node_id: String,
) -> anyhow::Result<()> {
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
                        response: format!("pong from {node_id}: {message}")
                    }),
                    error: None,
                }
            }
        };

        send_rpc_frame(&mut stream, &response).await?;
    }

    Ok(())
}
