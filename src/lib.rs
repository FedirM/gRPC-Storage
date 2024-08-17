pub mod db;
pub mod grpc;
pub mod models;
pub mod schema;

pub mod storage {
    tonic::include_proto!("storage");
}
