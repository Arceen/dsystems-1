use serde::{Deserialize, Serialize};
use crate::rpc::model::result::RpcResult;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RpcResponse {
    pub id: u64,
    pub result: Option<RpcResult>,
    pub error: Option<String>,
}
