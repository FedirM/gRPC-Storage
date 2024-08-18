use diesel::{
    pg::PgConnection,
    prelude::*,
    r2d2::{ConnectionManager, Pool},
    SelectableHelper,
};
use dotenvy::dotenv;
use log::error;
use std::{env, time::Duration};

use crate::{
    models::{NewStoreItem, StoreItem},
    schema::{
        store::dsl::*,
        store::{self, file_hash},
    },
};

pub type DbPool = Pool<ConnectionManager<PgConnection>>;

pub struct DbState {
    pub db_pool: DbPool,
}

impl DbState {
    pub fn new() -> Self {
        dotenv().ok();

        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let database_max_size: u32 = match env::var("DATABASE_POOL_MAX_SIZE") {
            Ok(val) => val.parse().unwrap_or_else(|_| {
                error!("0 < DATABASE_POOL_MAX_SIZE < {}", u32::MAX);
                panic!()
            }),
            Err(_) => 10,
        };

        let database_connection_timeout: u64 = match env::var("DATABASE_POOL_CONNECTION_TIMEOUT") {
            Ok(val) => val.parse().unwrap_or_else(|_| {
                error!(
                    "0 < DATABASE_POOL_CONNECTION_TIMEOUT (in sec) < {}",
                    u64::MAX
                );
                panic!()
            }),
            Err(_) => 30,
        };

        let database_idle_timeout: u64 = match env::var("DATABASE_POOL_IDLE_TIMEOUT") {
            Ok(val) => val.parse().unwrap_or_else(|_| {
                error!("0 < DATABASE_POOL_IDLE_TIMEOUT (in sec) < {}", u64::MAX);
                panic!()
            }),
            Err(_) => 600,
        };

        let connection_manager = ConnectionManager::<PgConnection>::new(database_url);

        match Pool::builder()
            .max_size(database_max_size)
            .connection_timeout(Duration::new(database_connection_timeout, 0))
            .idle_timeout(Some(Duration::new(database_idle_timeout, 0)))
            .build(connection_manager)
        {
            Ok(pool) => return Self { db_pool: pool },
            Err(e) => {
                error!("Couldn't create connection pool! Err: {}", e);
                panic!()
            }
        }
    }

    pub fn get_file_by_hash(&self, hash: String) -> Option<StoreItem> {
        let mut connection = self.db_pool.get().unwrap();

        match store
            .filter(file_hash.eq(hash))
            .select(StoreItem::as_select())
            .load(&mut connection)
        {
            Ok(mut list) => {
                if list.len() > 0 {
                    return Some(list.remove(0));
                } else {
                    return None;
                }
            }
            Err(_) => None,
        }
    }

    pub fn add_new_item(&self, item: &NewStoreItem) -> Result<StoreItem, diesel::result::Error> {
        let mut connection = self.db_pool.get().unwrap();

        match diesel::insert_into(store::table)
            .values(item)
            .returning(StoreItem::as_returning())
            .get_result(&mut connection)
        {
            Ok(res) => Ok(res),
            Err(e) => Err(e),
        }
    }

    pub fn update_last_read_state(
        &self,
        rec_id: i32,
        state: bool,
    ) -> Result<StoreItem, diesel::result::Error> {
        let mut connection = self.db_pool.get().unwrap();

        match diesel::update(store)
            .filter(id.eq(rec_id))
            .set(file_is_error.eq(state))
            .get_result::<StoreItem>(&mut connection)
        {
            Ok(item) => {
                return Ok(item);
            }
            Err(e) => Err(e),
        }
    }

    pub fn remove_item_by_hash(&self, hash: String) -> Result<StoreItem, diesel::result::Error> {
        let mut connection = self.db_pool.get().unwrap();

        match self.get_file_by_hash(hash) {
            Some(rec) => {
                diesel::delete(store.filter(id.eq(rec.id))).execute(&mut connection)?;
                Ok(rec)
            }
            None => Err(diesel::result::Error::NotFound),
        }
    }
}
