use serde::{Deserialize, Serialize};
use crate::rpc::model::params::RpcParams;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RpcRequest {
    pub id: u64,
    pub method: String,
    pub params: RpcParams,
}
