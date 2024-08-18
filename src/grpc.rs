use chrono::Utc;
use dotenvy::dotenv;
use log::{error, info, warn};
use sha2::{Digest, Sha256};
use std::{env, path::PathBuf};
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
        info!("Storage folder: {}", &env_dir);
        let dir = PathBuf::from(env_dir);

        let limit: u64 = env::var("CHUNK_SIZE_BYTES")
            .unwrap_or("1048576".to_owned())
            .parse()
            .unwrap_or_else(|_| {
                error!(
                    "'CHUNK_SIZE_BYTES' - should be an integer value in range: [1;{}]",
                    u64::MAX
                );
                panic!()
            });

        if limit < 1 || limit > u64::MAX {
            error!(
                "'CHUNK_SIZE_BYTES' - should be an integer value in range: [1;{}]",
                u64::MAX
            );
            panic!();
        }

        if dir.exists() && dir.is_dir() {
            return Self {
                db: DbState::new(),
                storage_folder: dir,
                chunk_size: limit,
            };
        } else {
            error!("'STORAGE_FOLDER' path - doesn't exists or it's not a directory!");
            panic!()
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

                        info!("Writing in file: {}", file_path.display());

                        let file = File::create(&file_path).await.map_err(|e| {
                            error!("Failed to create file: {}", &e);
                            Status::internal(format!("Failed to create file: {}", e))
                        })?;

                        file_handler = Some(file);
                    }
                    Data::Chunk(chunk_data) => {
                        if let Some(ref mut fh) = file_handler {
                            hasher.update(&chunk_data);
                            fh.write_all(&chunk_data).await.map_err(|e| {
                                error!("Failed to write data in file: {}", &e);
                                Status::internal(format!("Failed to write data in file: {}", e))
                            })?;
                        } else {
                            warn!("File name should be sent before chunks!");
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
            Err(e) => {
                error!("Error during adding new item to DB! Error: {}", &e);
                return Err(Status::new(tonic::Code::Internal, format!("{}", e)));
            }
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
                        Ok(res) => {
                            warn!(
                                "File \"{}\" with id:{} has problems with itself or path!",
                                &res.file_path, res.id
                            );
                            return Err(Status::new(
                                tonic::Code::NotFound,
                                format!("File not found!"),
                            ));
                        }
                        Err(_) => unreachable!(),
                    }
                }

                info!("Reading file {}", path.display());

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
                                error!("Error occured during sending chunk! Err: {}", err);
                                break;
                            }

                            if bytes_read < capacity as usize {
                                break;
                            }
                        }

                        Ok::<(), anyhow::Error>(())
                    };

                    if let Err(err) = result.await {
                        error!("{}", err);

                        let send_result = tx_error
                            .send(Err(Status::internal("Failed to send file")))
                            .await;

                        if let Err(err) = send_result {
                            error!("{}", err);
                        }
                    }
                });
                Ok(Response::new(ReceiverStream::new(rx)))
            }
            None => {
                error!("Could not found such hash!");
                return Err(Status::new(
                    tonic::Code::NotFound,
                    format!("Could not found such hash!"),
                ));
            }
        }
    }

    async fn delete_file(
        &self,
        request: Request<DeleteFileRequest>,
    ) -> Result<Response<DeleteFileResponse>, Status> {
        let request = request.into_inner();
        match self.db.remove_item_by_hash(request.file_hash.clone()) {
            Ok(item) => {
                let path = PathBuf::from(item.file_path);
                if !path.exists() {
                    if !item.file_is_error {
                        match self.db.update_last_read_state(item.id, true) {
                            Ok(res) => {
                                warn!("There is a problem with file \"{}\"", &res.file_path);
                                return Err(Status::new(
                                    tonic::Code::Internal,
                                    format!("Could not find file!"),
                                ));
                            }
                            Err(e) => {
                                error!("Could not update error state in DB! Error: {}", e);
                                return Err(Status::new(
                                    tonic::Code::Internal,
                                    format!("Internal service error!"),
                                ));
                            }
                        }
                    }
                    return Err(Status::new(
                        tonic::Code::NotFound,
                        format!("File not found!"),
                    ));
                }

                match remove_file(&path).await {
                    Ok(_) => {
                        info!("Delete: {}", path.display());
                        return Ok(Response::new(DeleteFileResponse {
                            code: tonic::Code::Ok as i32,
                            message: String::from("Ok"),
                        }));
                    }
                    Err(_) => match self.db.update_last_read_state(item.id, true) {
                        Ok(res) => {
                            warn!("There is a problem with file \"{}\"", &res.file_path);
                            return Err(Status::new(
                                tonic::Code::Internal,
                                format!("Internal service error!"),
                            ));
                        }
                        Err(e) => {
                            error!("Could not update error state in DB! Error: {}", e);
                            return Err(Status::new(
                                tonic::Code::Internal,
                                format!("Internal service error!"),
                            ));
                        }
                    },
                }
            }
            Err(_) => {
                error!("Could not found record with hash: {}", request.file_hash);
                return Err(Status::new(
                    tonic::Code::Internal,
                    format!("Record not found!"),
                ));
            }
        }
    }
}
