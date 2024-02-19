mod package;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Error, FromRow, migrate, query, query_as, Row, SqlitePool};
use sqlx::types::Json;
use anyhow::{Context, };
use serde::de::DeserializeOwned;
use sqlx::error::DatabaseError;
use sqlx::sqlite::{SqliteConnectOptions, SqliteRow};
use crate::database::package::PackageRecord;
use crate::package::Package;

const FILE: &str = "serene.db";

type Database = SqlitePool;

/// connects to the local sqlite database
pub async fn connect() -> anyhow::Result<Database> {

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
    fn create_record(&self) -> anyhow::Result<T>;
    fn from_record(other: T) -> anyhow::Result<Self> where Self: Sized;
}

trait MapDatabaseError<D> {
    fn db_error(self, column: &str) -> Result<D, Error>;
}

impl<D> MapDatabaseError<D> for serde_json::Result<D> {
    fn db_error(self, column: &str) -> Result<D, Error> {
        self.map_err(|e| Error::ColumnDecode { index: column.to_string(), source: Box::new(e) })
    }
}

trait AdvancedGet {
    fn try_get_utc(&self, index: &str) -> Result<DateTime<Utc>, Error>;
    fn try_get_json<D>(&self, index: &str) -> Result<D, Error> where D: DeserializeOwned;
}

impl AdvancedGet for SqliteRow {
    fn try_get_utc(&self, index: &str) -> Result<DateTime<Utc>, Error> {
        Ok(self.try_get::<NaiveDateTime, &str>(index)?.and_utc())
    }

    fn try_get_json<D>(&self, index: &str) -> Result<D, Error> where D: DeserializeOwned {
        let source = self.try_get::<String, &str>(index)?;
        serde_json::from_str(&source).map_err(|e|
            Error::ColumnDecode { index: index.to_owned(), source: Box::new(e) }
        )
    }
}


impl FromRow<'_, SqliteRow> for Package {
    fn from_row(row: &'_ SqliteRow) -> Result<Self, Error> {

        Ok(Self {
            base: row.try_get("base")?,
            added: row.try_get_utc("added")?,
            source: row.try_get_json("source")?,
            version: row.try_get("version")?,
            enabled: row.try_get("enabled")?,
            clean: row.try_get("clean")?,
            schedule: row.try_get("schedule")?,
            prepare: row.try_get("prepare")?,
            builds: vec![],
        })

    }
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
        }
    }
}