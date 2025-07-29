use crate::rpc::model::params::RpcParams;
use crate::rpc::model::request::RpcRequest;

pub fn parse_client_command(input: &str, id: u64) -> anyhow::Result<RpcRequest> {
    let parts: Vec<&str> = input.split_whitespace().collect();

    let params = match parts.as_slice() {
        ["get", key] => RpcParams::Get { key: key.to_string() },
        ["put", key, value @ ..] => {
            if value.is_empty() {
                anyhow::bail!("PUT requires a value");
            }
            RpcParams::Put {
                key: key.to_string(),
                value: value.join(" ")
            }
        },
        ["list"] => RpcParams::ListKeys,
        ["ping", message @ ..] => RpcParams::Ping {
            message: message.join(" ")
        },
        _ => anyhow::bail!("Unknown command"),
    };

    Ok(RpcRequest {
        id,
        method: match &params {
            RpcParams::Get { .. } => "get",
            RpcParams::Put { .. } => "put",
            RpcParams::ListKeys => "list",
            RpcParams::Ping { .. } => "ping",
        }.to_string(),
        params,
    })
}