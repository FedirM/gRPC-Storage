use dotenvy::dotenv;
use file_hash_service::{grpc::FileStorage, storage::storage_server::StorageServer};
use std::{env, net::SocketAddr};

use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let addr = env::var("SERVER_ADDR")
        .unwrap_or("[::1]:50051".to_string())
        .parse::<SocketAddr>()
        .expect(&format!("'SERVER_ADDR' - should be an IPv4/6 address"));

    println!("Server listening on {}", addr);

    Server::builder()
        .add_service(StorageServer::new(FileStorage::new()))
        .serve(addr)
        .await?;

    Ok(())
}
