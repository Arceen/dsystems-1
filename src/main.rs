use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use anyhow::{Result, Context};
use tokio::io::AsyncReadExt;
use serde::{Serialize, Deserialize};
use clap::{Parser, Subcommand};

#[derive(Debug, Clone, Serialize, Deserialize)]

#[derive(Serialize, Deserialize, Debug)]
struct Message {
    method: String,
    data: String,
}

#[tokio::main]
async fn main() -> Result<()>{
    let map:Arc<HashMap<String, &[u8]>> = Arc::new(HashMap::new());
    let socket = TcpListener::bind("127.0.0.1:3000").await.with_context(|| "somewhere we can go")?;

    // println!("{:#?}", v);

    loop {
        let ( stream, _) = socket.accept().await?;
        tokio::spawn(handle_connection(stream, map.clone()));
    }
}

async fn handle_connection(mut stream: TcpStream, map: Arc<HashMap<String, &[u8]>> ) -> Result<()>{
    loop {
        let mut len_bytes: [u8; 4] = [0; 4];
        println!("waiting to read bytes");
        stream.read_exact(&mut len_bytes).await?;
        let len = u32::from_be_bytes(len_bytes) as usize;
        println!("Read some bytes");
        let mut data = vec![0u8; len];
        stream.read_exact(&mut data).await?;
        // serde_json::from_str(r#"0"#
        let msg: Message = serde_json::from_str(str::from_utf8(&data)?)?;
        println!("{:#?}", msg);

    }
}

