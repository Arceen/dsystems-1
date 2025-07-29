use std::collections::HashMap;
use serde::{Deserialize, Serialize};

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
