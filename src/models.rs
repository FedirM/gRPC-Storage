use crate::schema::store;
use diesel::prelude::*;

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = store)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct StoreItem {
    pub id: i32,
    pub file_name: String,
    pub file_path: String,
    pub file_hash: String,
    pub file_is_error: bool,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = store)]
pub struct NewStoreItem {
    pub file_name: String,
    pub file_path: String,
    pub file_hash: String,
}
