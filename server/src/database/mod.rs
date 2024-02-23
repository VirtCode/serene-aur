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

pub type Database = SqlitePool;

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