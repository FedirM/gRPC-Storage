use file_hash_service::storage::{
    storage_client::StorageClient, DeleteFileRequest, FetchFileRequest, UploadFileRequest,
};
use std::{
    env,
    fs::File,
    io::{Read, Write},
};
use tonic::transport::Channel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let mut client = StorageClient::connect(format!("http://{}", env::var("SERVER_ADDR")?)).await?;

    // Example Usage:
    let command = env::args().nth(1).expect("No command provided");

    match command.as_str() {
        "upload" => {
            let file_path = env::args().nth(2).expect("No file path provided");
            upload_file(&mut client, file_path).await?;
        }
        "fetch" => {
            let file_hash = env::args().nth(2).expect("No file hash provided");
            fetch_file(&mut client, file_hash, env::args().nth(3)).await?;
        }
        "delete" => {
            let file_hash = env::args().nth(2).expect("No file hash provided");
            delete_file(&mut client, file_hash).await?;
        }
        "-h" | "--help" => print_help(),
        _ => {
            println!("Unknown command. Use 'upload', 'fetch', or 'delete'.");
            print_help();
        }
    }

    Ok(())
}

fn print_help() {
    println!("Usage:");
    println!("  upload <file_path>    - Upload a file");
    println!("  fetch <file_hash>     - Fetch a file by its hash");
    println!("  delete <file_hash>    - Delete a file by its hash");
}

async fn upload_file(
    client: &mut StorageClient<Channel>,
    file_path: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::open(&file_path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    let file_name = file_path.split("/").last().unwrap().to_string();

    let stream = tokio_stream::iter(vec![
        UploadFileRequest {
            data: Some(file_hash_service::storage::upload_file_request::Data::FileName(file_name)),
        },
        UploadFileRequest {
            data: Some(file_hash_service::storage::upload_file_request::Data::Chunk(buffer)),
        },
    ]);

    let response = client.upload_file(tonic::Request::new(stream)).await?;
    println!("File uploaded: {:?}", response.into_inner());

    Ok(())
}

async fn fetch_file(
    client: &mut StorageClient<Channel>,
    file_hash: String,
    file_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .fetch_file(FetchFileRequest { file_hash })
        .await?
        .into_inner();

    let mut file = if file_name.is_some() {
        File::create(file_name.unwrap())?
    } else {
        File::create("downloaded_file.txt")?
    };
    let mut stream = response;

    while let Some(chunk) = stream.message().await? {
        file.write_all(&chunk.chunk)?;
    }

    println!("Complete!");

    Ok(())
}

async fn delete_file(
    client: &mut StorageClient<Channel>,
    file_hash: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .delete_file(DeleteFileRequest { file_hash })
        .await?
        .into_inner();

    println!("Delete response: {:?}", response);

    Ok(())
}
