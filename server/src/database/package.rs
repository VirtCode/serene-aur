use anyhow::{Context};
use chrono::NaiveDateTime;
use sqlx::{query, query_as};
use crate::database::{Database, DatabaseConversion, MapDatabaseError};
use crate::package::Package;

#[derive(Debug)]
pub(super) struct PackageRecord {
    pub base: String,
    pub added: NaiveDateTime,
    pub source: String,
    pub srcinfo: Option<String>,
    pub pkgbuild: Option<String>,
    pub version: Option<String>,
    pub enabled: bool,
    pub clean: bool,
    pub schedule: Option<String>,
    pub prepare: Option<String>
}

impl DatabaseConversion<PackageRecord> for Package {
    fn create_record(&self) -> anyhow::Result<PackageRecord> {
        Ok(PackageRecord {
            base: self.base.clone(),
            added: self.added.naive_utc(),
            source: serde_json::to_string(&self.source).db_error("source")?,
            srcinfo: None,
            pkgbuild: None,
            version: Some(self.version.clone()),
            enabled: self.enabled,
            clean: self.clean,
            schedule: self.schedule.clone(),
            prepare: self.prepare.clone()
        })
    }

    fn from_record(value: PackageRecord) -> anyhow::Result<Self> where Self: Sized {
        Ok(Self {
            base: value.base,
            added: value.added.and_utc(),
            source: serde_json::from_str(&value.source).db_error("source")?,
            version: value.version.unwrap_or("unknown".to_owned()), // TODO: represent as option in Package
            enabled: value.enabled,
            clean: value.clean,
            schedule: value.schedule,
            prepare: value.prepare,
            builds: vec![]
        })
    }
}

impl Package {
    pub async fn find(base: &str, db: &Database) -> anyhow::Result<Self> {
        let record = query_as!(PackageRecord, r#"
            SELECT * FROM package WHERE base = $1
        "#,
            base
        )
            .fetch_one(db).await?;

        Package::from_record(record)
    }

    pub async fn find_all(db: &Database) -> anyhow::Result<Vec<Self>> {
        let records = query_as!(PackageRecord, r#"
            SELECT * FROM package
        "#)
            .fetch_all(db).await?;

        records.into_iter().map(|r| Package::from_record(r)).collect()
    }

    pub async fn save(&self, db: &Database) -> anyhow::Result<()> {
        let record = self.create_record().context("failed to convert package to record")?;

        query!(r#"
            INSERT INTO package (base, added, source, srcinfo, pkgbuild, version, enabled, clean, schedule, prepare)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
            record.base, record.added, record.source, record.srcinfo, record.pkgbuild, record.version, record.enabled, record.clean, record.schedule, record.prepare
        )
            .execute(db).await?;

        Ok(())
    }

    pub async fn update(&self, db: &Database) -> anyhow::Result<()> {
        let record = self.create_record().context("failed to convert package to record")?;

        query!(r#"
            UPDATE package
            SET source = $2, srcinfo = $3, pkgbuild = $4, version = $5, enabled = $6, clean = $7, schedule = $8, prepare = $9
            WHERE base = $1
        "#,
            record.base, record.source, record.srcinfo, record.pkgbuild, record.version, record.enabled, record.clean, record.schedule, record. prepare
        )
            .execute(db).await?;

        Ok(())
    }

    pub async fn delete(&self, db: &Database) -> anyhow::Result<()> {
        let base = &self.base;

        query!(r#"
            DELETE FROM package WHERE base = $1
        "#,
            base
        )
            .execute(db).await?;

        Ok(())
    }

}