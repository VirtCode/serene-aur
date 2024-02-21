mod package;
mod build;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Error, FromRow, migrate, query, query_as, Row, SqlitePool};
use sqlx::types::Json;
use anyhow::{Context, };
use serde::de::DeserializeOwned;
use sqlx::error::DatabaseError;
use sqlx::sqlite::{SqliteConnectOptions, SqliteRow};
use anyhow::Result;

const FILE: &str = "serene.db";

type Database = SqlitePool;

/// connects to the local sqlite database
pub async fn connect() -> Result<Database> {

    // connecting
    let pool = SqlitePool::connect_with(
        SqliteConnectOptions::new()
            .filename(FILE)
            .foreign_keys(true)
            .create_if_missing(true)
    ).await.context("failed to connect to database")?;

    // running migrations
    migrate!().run(&pool).await
        .context("failed to migrate database")?;

    Ok(pool)
}

trait DatabaseConversion<T> {
    fn create_record(&self) -> Result<T>;
    fn from_record(other: T) -> Result<Self> where Self: Sized;
}

#[cfg(test)]
mod tests {
    use std::env;
    use crate::package::store::PackageStore;
    use super::*;

    #[tokio::test]
    pub async fn insert_local() {
        env::set_current_dir("../app/").unwrap();

        let db = connect().await.unwrap();

        let store = PackageStore::init().await
            .context("failed to create serene data storage").unwrap();

        for x in store.peek() {
            x.save(&db).await.unwrap();
            for b in &x.builds {
                b.save(&db).await.unwrap();
            }
        }
    }
}