use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

pub async fn send_rpc_frame<T: Serialize>(stream: &mut TcpStream, obj: &T) -> anyhow::Result<()> {
    let json_str = serde_json::to_string(obj)
        .map_err(|e| anyhow::anyhow!("Failed to serialize to JSON: {}", e))?;

    println!("Serialized version: {json_str}");
    let json_bytes = json_str.as_bytes();
    let len = json_bytes.len() as u32;

    stream.write_all(&len.to_be_bytes()).await
        .map_err(|e| anyhow::anyhow!("Failed to write frame length: {}", e))?;
    stream.write_all(json_bytes).await
        .map_err(|e| anyhow::anyhow!("Failed to write frame data: {}", e))?;

    Ok(())
}