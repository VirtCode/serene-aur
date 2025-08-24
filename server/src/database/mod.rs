pub mod build;
pub mod log;
pub mod package;

use ::log::info;
use anyhow::Context;
use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::{migrate, SqlitePool};

const FILE: &str = "serene.db";

pub type Database = SqlitePool;

/// connects to the local sqlite database
pub async fn connect() -> Result<Database> {
    info!("connecting to the database");

    // connecting
    let pool = SqlitePool::connect_with(
        SqliteConnectOptions::new()
            .filename(FILE)
            .foreign_keys(true)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal),
    )
    .await
    .context("failed to connect to database")?;

    // running migrations
    migrate!().run(&pool).await.context("failed to migrate database")?;

    Ok(pool)
}

trait DatabaseConversion<T> {
    fn create_record(&self) -> Result<T>;
    fn from_record(other: T) -> Result<Self>
    where
        Self: Sized;
}
