use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NodeInfo {
    pub id: String,
    pub address: String,
    pub status: NodeStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum NodeStatus {
    Active,
    Inactive,
}
