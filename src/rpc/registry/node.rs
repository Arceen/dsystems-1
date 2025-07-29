use std::sync::Arc;
use tokio::net::TcpStream;
use crate::rpc::model::node::NodeStatus;
use crate::rpc::model::service::ServiceRegistry;

pub async fn check_node_health(address: &str) -> bool {
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

pub async fn update_node_status(registry: Arc<tokio::sync::RwLock<ServiceRegistry>>, node_id: String, is_healthy: bool) {
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
