# gRPC-Storage

This project is a simple yet robust microservice designed for efficient file storage and retrieval. It allows users to upload, store, and access files of any type and size with ease. The service utilizes a PostgreSQL database to manage file metadata, including the file name, file path, and a unique hash (SHA-256) for quick and secure access. The actual files are stored in a designated folder on the server, ensuring that the storage is both organized and scalable.

### Key Features

- File Upload: Upload files and store them securely on the server. The service calculates a SHA-256 hash for each file, ensuring a unique identifier for every file.
- File Retrieval: Quickly retrieve files using their unique hash, making access both efficient and secure.
- File Deletion: Easily delete files, with the service ensuring that both the file and its metadata are removed.
- Chunked Data Handling: Supports large file uploads and retrievals by handling data in configurable chunks.
- PostgreSQL Integration: All file metadata is stored in a PostgreSQL database, allowing for easy management and querying of stored files.

### Project Structure

```sh
git:gRPC-Storage/
    ├
    ├── build.rs                <-- Main build script to compile .proto file(s)
    ├── ...
    ├── migrations              <-- Diesele migration schemes
    ├── src
    │   ├── db.rs               <-- DB handlers
    │   ├── grpc.rs             <-- Tonic grpc server methods
    │   ├── main.rs             <-- Entry point / start micro-service
    │   └── ...
    └── usage-example
        └── cli-client.rs       <-- Simple CLI client to demonstrate basic usage
```

## Getting Started (SERVER)

1. Based on `[PROJECT]/.env.example ` file create a new one without `.example` suffix and fill up according your environment!
2. Ensure your Postgres agent is up!
3. Install diesel-cli if not! [Here is how to do that.](https://github.com/diesel-rs/diesel/tree/2.2.x/diesel_cli)
4. Run diesel migration:

```sh
> diesel migration run
```

5. Simply run the server:

```sh
> cargo run --bin grpc-server
```

## Usage

### Test purpose

Just to check how this work, you could easily setup POSTMAN (or any another gRPC request's compatible) client. [POSTMAN gRPC setup guide.](https://learning.postman.com/docs/sending-requests/grpc/first-grpc-request/)

### [Example (CLI client)](https://github.com/FedirM/gRPC-Storage/blob/master/usage-example/cli-client.rs)

[CLI client](https://github.com/FedirM/gRPC-Storage/blob/master/usage-example/cli-client.rs) allows you to interact with the gRPC-Storage microservice to upload, fetch, or delete files. Below are the available commands and their usage.

#### Commands

- Upload a file to the storage:

```
> cargo run --bin client -- upload <file_path>
```

- Fetch file from the storage:

```
> cargo run --bin client -- fetch <file_hash> [output_file]
```

- Delete a File

```
> cargo run --bin client -- delete <file_hash>
```

- Display the help message.

```
> cargo run --bin client -- -h
    or
> cargo run --bin client -- --help
```
