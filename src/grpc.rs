use chrono::Utc;
use dotenvy::dotenv;
use sha2::{Digest, Sha256};
use std::env;
use std::path::PathBuf;
use tokio::{
    fs::{remove_file, File},
    io::{AsyncReadExt, AsyncWriteExt},
    sync::mpsc,
};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};

use crate::{
    db::DbState,
    models::NewStoreItem,
    storage::{
        storage_server::Storage, upload_file_request::Data, DeleteFileRequest, DeleteFileResponse,
        FetchFileRequest, FetchFileResponse, UploadFileRequest, UploadFileResponse,
    },
};

pub struct FileStorage {
    db: DbState,
    storage_folder: PathBuf,
    chunk_size: u64, //in bytes
}

impl FileStorage {
    pub fn new() -> Self {
        dotenv().ok();
        let env_dir =
            env::var("STORAGE_FOLDER").unwrap_or(env::current_dir().unwrap().display().to_string());
        println!("Storage folder: {}", &env_dir);
        let dir = PathBuf::from(env_dir);

        let limit: u64 = env::var("CHUNK_SIZE_BYTES")
            .unwrap_or("1048576".to_owned())
            .parse()
            .expect(&format!(
                "'CHUNK_SIZE_BYTES' - should be an integer value in range: [1;{}]",
                u64::MAX
            ));

        if limit < 1 || limit > u64::MAX {
            panic!(
                "{}",
                format!(
                    "'CHUNK_SIZE_BYTES' - should be an integer value in range: [1;{}]",
                    u64::MAX
                )
            );
        }

        if dir.exists() && dir.is_dir() {
            return Self {
                db: DbState::new(),
                storage_folder: dir,
                chunk_size: limit,
            };
        } else {
            panic!("'STORAGE_FOLDER' path - doesn't exists or it's not a directory!")
        }
    }
}

#[tonic::async_trait]
impl Storage for FileStorage {
    type FetchFileStream = ReceiverStream<Result<FetchFileResponse, Status>>;

    async fn upload_file(
        &self,
        request: Request<Streaming<UploadFileRequest>>,
    ) -> Result<Response<UploadFileResponse>, Status> {
        let mut stream = request.into_inner();

        let mut file_name: Option<String> = None;
        let mut file_handler: Option<File> = None;
        let mut file_path = self.storage_folder.clone();

        let file_hash: Option<String>;
        let mut hasher = Sha256::new();

        while let Some(chunk) = stream.message().await? {
            if let Some(data) = chunk.data {
                match data {
                    Data::FileName(name) => {
                        file_name = Some(name.clone());

                        file_path.push(format!(
                            "{}_{}",
                            Utc::now().timestamp_millis(),
                            file_name.clone().unwrap()
                        ));

                        println!("Filepath: {}", file_path.display());

                        let file = File::create(&file_path).await.map_err(|e| {
                            Status::internal(format!("Failed to create file: {}", e))
                        })?;

                        file_handler = Some(file);
                    }
                    Data::Chunk(chunk_data) => {
                        if let Some(ref mut fh) = file_handler {
                            hasher.update(&chunk_data);
                            fh.write_all(&chunk_data).await.map_err(|e| {
                                Status::internal(format!("Failed to write data in file: {}", e))
                            })?;
                        } else {
                            return Err(Status::internal(format!(
                                "File name didn't specified yet!"
                            )));
                        }
                    }
                }
            }
        }

        if file_handler.is_some() {
            file_handler.unwrap().sync_all().await.unwrap();
        }

        file_hash = Some(format!("{:x}", hasher.finalize()));

        match self.db.add_new_item(&NewStoreItem {
            file_name: file_name.unwrap(),
            file_path: file_path.display().to_string(),
            file_hash: file_hash.unwrap(),
        }) {
            Ok(res) => Ok(Response::new(UploadFileResponse {
                file_name: res.file_name,
                file_hash: res.file_hash,
            })),
            Err(e) => Err(Status::new(tonic::Code::Internal, format!("{:?}", e))),
        }
    }

    async fn fetch_file(
        &self,
        request: Request<FetchFileRequest>,
    ) -> Result<Response<Self::FetchFileStream>, Status> {
        let req = request.into_inner();

        match self.db.get_file_by_hash(req.file_hash) {
            Some(res) => {
                let path = PathBuf::from(res.file_path);
                if !path.exists() || !path.is_file() {
                    match self.db.update_last_read_state(res.id, true) {
                        Ok(_) => {
                            return Err(Status::new(
                                tonic::Code::NotFound,
                                format!("File not found!"),
                            ));
                        }
                        Err(_) => unreachable!(),
                    }
                }

                println!("Reading file {}", path.display());

                let (tx, rx) = mpsc::channel(self.chunk_size as usize);
                let tx_error = tx.clone();
                let capacity = self.chunk_size;

                tokio::spawn(async move {
                    let result = async move {
                        let fh = File::open(path).await.unwrap();
                        let mut handler = fh.take(capacity);

                        loop {
                            let mut response = FetchFileResponse {
                                chunk: Vec::with_capacity(capacity as usize),
                            };

                            let bytes_read = handler.read_to_end(&mut response.chunk).await?;

                            if bytes_read == 0 {
                                break;
                            } else {
                                handler.set_limit(capacity);
                            }

                            if let Err(err) = tx.send(Ok(response)).await {
                                eprintln!("Send error: {}", err);
                                break;
                            }

                            if bytes_read < capacity as usize {
                                break;
                            }
                        }

                        Ok::<(), anyhow::Error>(())
                    };

                    if let Err(err) = result.await {
                        eprintln!("{}", err);

                        let send_result = tx_error
                            .send(Err(Status::internal("Failed to send file")))
                            .await;

                        if let Err(err) = send_result {
                            eprintln!("{}", err);
                        }
                    }
                });
                Ok(Response::new(ReceiverStream::new(rx)))
            }
            None => Err(Status::new(
                tonic::Code::NotFound,
                format!("Could not found such hash!"),
            )),
        }
    }

    async fn delete_file(
        &self,
        request: Request<DeleteFileRequest>,
    ) -> Result<Response<DeleteFileResponse>, Status> {
        let request = request.into_inner();
        match self.db.remove_item_by_hash(request.file_hash) {
            Ok(item) => {
                let path = PathBuf::from(item.file_path);
                if !path.exists() {
                    return Err(Status::new(
                        tonic::Code::NotFound,
                        format!("File not found!"),
                    ));
                }

                match remove_file(path).await {
                    Ok(_) => {
                        return Ok(Response::new(DeleteFileResponse {
                            code: tonic::Code::Ok as i32,
                            message: String::from("Ok"),
                        }))
                    }
                    Err(_) => match self.db.update_last_read_state(item.id, true) {
                        Ok(_) => {
                            return Err(Status::new(
                                tonic::Code::Internal,
                                format!("Internal service error!"),
                            ))
                        }
                        Err(_) => Err(Status::new(
                            tonic::Code::Internal,
                            format!("Record not found!"),
                        )),
                    },
                }
            }
            Err(_) => Err(Status::new(
                tonic::Code::Internal,
                format!("Record not found!"),
            )),
        }
    }
}
