use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::rpc::model::node::NodeInfo;

#[derive(Serialize, Deserialize, Debug)]
pub struct ServiceRegistry {
    pub nodes: HashMap<String, NodeInfo>,
}
