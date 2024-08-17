-- Your SQL goes here
CREATE TABLE store (
    id SERIAL PRIMARY KEY,
    file_name VARCHAR NOT NULL,
    file_path VARCHAR NOT NULL,
    file_hash VARCHAR NOT NULL,
    file_is_error BOOLEAN NOT NULL DEFAULT FALSE
)