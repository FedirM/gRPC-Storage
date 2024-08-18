use dotenvy::dotenv;
use log::info;
use std::{env, net::SocketAddr};
use tonic::transport::Server;

use grpc_storage::{grpc::FileStorage, storage::storage_server::StorageServer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }

    env_logger::builder()
        .format_timestamp(Some(env_logger::TimestampPrecision::Seconds))
        .try_init()?;

    let addr = env::var("SERVER_ADDR")
        .unwrap_or("[::1]:50051".to_string())
        .parse::<SocketAddr>()
        .expect(&format!("'SERVER_ADDR' - should be an IPv4/6 address"));

    info!("Server listening on {}", addr);

    Server::builder()
        .add_service(StorageServer::new(FileStorage::new()))
        .serve(addr)
        .await?;

    Ok(())
}
