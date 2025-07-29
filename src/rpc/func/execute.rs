use tokio::net::TcpStream;
use crate::rpc::func::frame::read::read_rpc_frame;
use crate::rpc::func::frame::write::send_rpc_frame;
use crate::rpc::model::request::RpcRequest;
use crate::rpc::model::response::RpcResponse;

pub async fn execute_rpc_call(node_addr: &str, request: RpcRequest) -> anyhow::Result<RpcResponse> {
    let mut stream = TcpStream::connect(node_addr).await?;
    send_rpc_frame(&mut stream, &request).await?;
    read_rpc_frame(&mut stream).await
}
