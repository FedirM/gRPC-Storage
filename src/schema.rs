// @generated automatically by Diesel CLI.

diesel::table! {
    store (id) {
        id -> Int4,
        file_name -> Varchar,
        file_path -> Varchar,
        file_hash -> Varchar,
        file_is_error -> Bool,
    }
}
