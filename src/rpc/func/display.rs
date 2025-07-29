use crate::rpc::model::response::RpcResponse;
use crate::rpc::model::result::RpcResult;

pub fn display_response(response: RpcResponse) {
    match response.result {
        Some(RpcResult::GetResult { value }) => {
            match value {
                Some(v) => println!("✅ Found: {v}"),
                None => println!("❌ Key not found"),
            }
        }
        Some(RpcResult::PutResult { success }) => {
            if success {
                println!("✅ Value stored successfully");
            } else {
                println!("❌ Failed to store value");
            }
        }
        Some(RpcResult::ListResult { keys }) => {
            println!("📋 Keys ({}):", keys.len());
            for key in keys {
                println!("  - {key}");
            }
        }
        Some(RpcResult::PingResult { response }) => {
            println!("🏓 {response}");
        }
        None => {
            if let Some(error) = response.error {
                println!("❌ Error: {error}");
            }
        }
    }
}

