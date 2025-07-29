use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum RpcResult {
    GetResult { value: Option<String> },
    PutResult { success: bool },
    ListResult { keys: Vec<String> },
    PingResult { response: String },
}
