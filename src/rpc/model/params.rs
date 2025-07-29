use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "method", content = "args")]
pub enum RpcParams {
    Get { key: String },
    Put { key: String, value: String },
    ListKeys,
    Ping { message: String },
}
