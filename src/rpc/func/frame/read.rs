use serde::Deserialize;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

// RPC Protocol Implementation
pub async fn read_rpc_frame<T: for<'de> Deserialize<'de>>(stream: &mut TcpStream) -> anyhow::Result<T> {
    let mut len_bytes = [0u8; 4];
    stream.read_exact(&mut len_bytes).await
        .map_err(|e| anyhow::anyhow!("Failed to read frame length: {}", e))?;
    let len = u32::from_be_bytes(len_bytes) as usize;

    if len > 1024 * 1024 {
        anyhow::bail!("Frame too large: {} bytes", len);
    }

    let mut data = vec![0u8; len];
    stream.read_exact(&mut data).await
        .map_err(|e| anyhow::anyhow!("Failed to read frame data ({} bytes): {}", len, e))?;

    let json_str = std::str::from_utf8(&data)
        .map_err(|e| anyhow::anyhow!("Frame data is not valid UTF-8: {}", e))?;

    println!("client Got this: {json_str}");
    let obj: T = serde_json::from_str(json_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse JSON: {} | Data: {}", e, json_str))?;

    Ok(obj)
}
