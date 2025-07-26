use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use anyhow::{Result, Context};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use serde::{Serialize, Deserialize};
use clap::{Parser, Subcommand};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
enum Operation {
    Get(String),
    Put(String, String),
    Ping(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Message {
    operation: Operation,
}

#[derive(Serialize, Deserialize, Debug)]
struct Response {
    success: bool,
    data: Option<String>,
    message: String,
}

#[derive(Parser)]
#[command(name = "dsys-1")]
#[command(about = "tcp client with get/set store")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>
}

#[derive(Subcommand)]
enum Commands {
    Client {
        #[arg(long, default_value="127.0.0.1:3000")]
        addr: String,
    }
}

type DistributedMap = Arc<RwLock<HashMap<String, String>>>;

#[tokio::main]
async fn main() -> Result<()>{
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Client {addr}) => {
            run_client(addr).await
        }
        None => {
            run_server().await
        }
    }

}

async fn run_server() -> Result<()> {

    let map:DistributedMap = Arc::new(RwLock::new(HashMap::new()));
    let socket = TcpListener::bind("127.0.0.1:3000")
        .await
        .context( "somewhere we can go")?;

    println!("🚀 Server listening on 127.0.0.1:3000");
    println!("📝 Supported operations: GET, PUT, PING");
    println!("🔧 Use 'cargo run -- client' to connect");

    loop {
        let ( stream, addr) = socket.accept()
            .await
            .context("Failed to accept connection")?;
        let cloned_map = map.clone();

        tokio::spawn(async move {
            if let Err(e) = tokio::spawn(handle_connection(stream, cloned_map)).await
                {
                    eprintln!("❌ Connection error: {:?}", e);
                }

        });
    }
}

async fn run_client(addr: String) -> Result <()> {
    println!("🔗 Connecting to {}", addr);
    let mut stream = TcpStream::connect(&addr)
        .await
        .with_context(|| format!("Failed to connect to {}", addr))?;

    println!("✅ Connected! Enter commands:");
    println!("📖 Commands:");
    println!("  get <key>           - Get value for key");
    println!("  put <key> <value>   - Set key to value");
    println!("  ping <message>      - Send ping message");
    println!("  q                   - Quit");
    println!();

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    loop {
        print!("💬 > ");
        use std::io::Write;
        std::io::stdout().flush()?;

        line.clear();
        reader.read_line(&mut line)
            .await
            .context("Failed to read from stdin")?;

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.to_ascii_lowercase() == "q" || trimmed.to_ascii_lowercase() == "quit" {
            println!("👋 Goodbye!");
            break;
        }

        match parse_command(trimmed) {
            Ok(operation) => {
                if let Err(e) = send_operation(&mut stream, operation).await {
                    eprintln!("❌ Failed to send command: {:?}", e);
                    break;
                }
            }
            Err(e) => {
                println!("❌ Invalid command: {}", e);
                continue;
            }
        }
    }
    Ok(())
}

fn parse_command(input: &str) -> Result<Operation> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    match parts.as_slice() {
        ["get", key] => Ok(Operation::Get(key.to_string()
        )),
        ["put", key, value @ ..] => {
            if value.is_empty() {
                anyhow::bail!("PUT requires a value. Usage: put <key> <value>");
            }
            Ok(Operation::Put(key.to_string(), value.join(" ")))
        },
        ["ping", message @ ..] => {
            let msg = if message.is_empty() {
                "ping".to_string()
            } else {
                message.join(" ")
            };
            Ok(Operation::Ping (msg))
        },
        _ => anyhow::bail!("Unknown command. Use: get <key>, put <key> <value>, ping <message>, or q"),
    }
}

async fn send_operation(stream: &mut TcpStream, operation: Operation) -> Result<()> {
    let message = Message { operation: operation.clone() };
    println!("📤 Sending: {:?}", operation);

    let json_str = serde_json::to_string(&message)
        .context("Failed to serialize message")?;

    let json_bytes = json_str.as_bytes();
    let len = json_bytes.len() as u32;

    stream.write_all(&len.to_be_bytes())
        .await
        .context("Failed to write frame length")?;

    stream.write_all(json_bytes)
        .await
        .context("Failed to write frame payload");


    match tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        read_response(stream)
    ).await {
        Ok(Ok(response)) => {
            if response.success {
                match response.data {
                    Some(data) => println!("✅ Success: {}", data),
                    None => println!("✅ {}", response.message),
                }
            } else {
                println!("❌ Error: {}", response.message);
            }
        }
        Ok(Err(e)) => println!("❌ Failed to read response: {:?}", e),
        Err(_) => println!("⏰ Server response timeout"),
    }
    Ok(())

}


async fn read_response(stream: &mut TcpStream) -> Result<Response> {
    let mut len_bytes = [0u8; 4];
    stream.read_exact(&mut len_bytes).await?;
    let len = u32::from_be_bytes(len_bytes) as usize;

    let mut data = vec![0u8; len];
    stream.read_exact(&mut data).await?;

    let json_str = std::str::from_utf8(&data)?;
    let response: Response = serde_json::from_str(json_str)?;

    Ok(response)
}
async fn handle_connection(mut stream: TcpStream, map: DistributedMap) -> Result<()> {
    loop {
        let mut len_bytes: [u8; 4] = [0; 4];

        match stream.read_exact(&mut len_bytes).await {
            Ok(_) => {},
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                println!("🔌 Client disconnected");
                break;
            }
            Err(e) => return Err(e).context("Failed to read frame length"),
        }

        let len = u32::from_be_bytes(len_bytes) as usize;

        if len > 1024 * 1024 {
            anyhow::bail!("Frame too large: {} bytes", len);
        }

        let mut data = vec![0u8; len];
        stream.read_exact(&mut data)
            .await
            .with_context(|| format!("Failed to read frame payload ({} bytes)", len))?;

        let json_str = std::str::from_utf8(&data)
            .context("Frame payload is not valid UTF-8")?;

        let msg: Message = serde_json::from_str(json_str)
            .with_context(|| format!("Failed to parse JSON: {}", json_str))?;

        println!("📨 Received: {:?}", msg.operation);

        // Process the operation and send response
        let response = process_operation(&msg.operation, &map).await;
        send_response(&mut stream, response).await?;
    }

    Ok(())
}


async fn process_operation(operation: &Operation, map: &Arc<tokio::sync::RwLock<HashMap<String, String>>>) -> Response {
    match operation {
        Operation::Get ( key ) => {
            let map_read = map.read().await;
            match map_read.get(key) {
                Some(value) => Response {
                    success: true,
                    data: Some(value.clone()),
                    message: format!("Found value for key '{}'", key),
                },
                None => Response {
                    success: false,
                    data: None,
                    message: format!("Key '{}' not found", key),
                },
            }
        }
        Operation::Put ( key, value ) => {
            let mut map_write = map.write().await;
            map_write.insert(key.clone(), value.clone());
            Response {
                success: true,
                data: None,
                message: format!("Set '{}' = '{}'", key, value),
            }
        }
        Operation::Ping ( message ) => {
            Response {
                success: true,
                data: Some(format!("pong: {}", message)),
                message: "Ping successful".to_string(),
            }
        }
    }
}

async fn send_response(stream: &mut TcpStream, response: Response) -> Result<()> {
    let json_str = serde_json::to_string(&response)?;
    let json_bytes = json_str.as_bytes();
    let len = json_bytes.len() as u32;

    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(json_bytes).await?;

    Ok(())
}