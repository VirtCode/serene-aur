use std::str::FromStr;
use chrono::NaiveDateTime;
use sqlx::{query, query_as};
use crate::database::{Database, DatabaseConversion };
use crate::package::Package;
use anyhow::{Context, Result};
use crate::package::source::SrcinfoWrapper;

/// See migrations:
/// server/migrations/20240210163236_package.sql,
/// server/migrations/20240306195926_makepkg_flags.sql
#[derive(Debug)]
struct PackageRecord {
    /// id
    base: String,

    added: NaiveDateTime,
    source: String,
    srcinfo: Option<String>,
    pkgbuild: Option<String>,
    version: Option<String>,
    enabled: bool,
    clean: bool,
    dependency: bool,
    schedule: Option<String>,
    prepare: Option<String>,
    flags: Option<String>
}

impl DatabaseConversion<PackageRecord> for Package {
    fn create_record(&self) -> Result<PackageRecord> {
        Ok(PackageRecord {
            base: self.base.clone(),
            added: self.added.naive_utc(),
            source: serde_json::to_string(&self.source).context("failed to serialize source")?,
            srcinfo: self.srcinfo.as_ref().map(|s| s.to_string()),
            pkgbuild: self.pkgbuild.clone(),
            version: self.version.clone(),
            enabled: self.enabled,
            clean: self.clean,
            schedule: self.schedule.clone(),
            prepare: self.prepare.clone(),
            flags: if !self.flags.is_empty() { Some(serde_json::to_string(&self.flags).context("failed to serialize flags")?) } else { None },
            dependency: self.dependency
        })
    }

    fn from_record(value: PackageRecord) -> Result<Package> where Self: Sized {
        Ok(Self {
            base: value.base,
            added: value.added.and_utc(),
            source: serde_json::from_str(&value.source).context("failed to deserialize source")?,
            version: value.version,
            pkgbuild: value.pkgbuild,
            srcinfo: value.srcinfo.map(|a| SrcinfoWrapper::from_str(&a)).transpose()?,
            enabled: value.enabled,
            clean: value.clean,
            schedule: value.schedule,
            prepare: value.prepare,
            flags: value.flags.map(|s| serde_json::from_str(&s).context("failed to deserialize source")).unwrap_or_else(|| Ok(vec![]))?,
            dependency: value.dependency,
        })
    }
}

impl Package {

    /// Returns whether the database contains a specific package
    pub async fn has(base: &str, db: &Database) -> Result<bool> {
        let amount = query!(r#"
            SELECT COUNT(base) as count FROM package WHERE base == $1
        "#,
            base
        )
            .fetch_one(db).await?.count;

        Ok(amount > 0)
    }

    /// Find a specific package from the database
    pub async fn find(base: &str, db: &Database) -> Result<Option<Self>> {
        let record = query_as!(PackageRecord, r#"
            SELECT * FROM package WHERE base = $1
        "#,
            base
        )
            .fetch_optional(db).await?;

        record.map(Package::from_record).transpose()
    }

    /// Find all packages from the database
    pub async fn find_all(db: &Database) -> Result<Vec<Self>> {
        let records = query_as!(PackageRecord, r#"
            SELECT * FROM package
        "#)
            .fetch_all(db).await?;

        records.into_iter().map(Package::from_record).collect()
    }

    /// Saves the package to the database for a first time
    pub async fn save(&self, db: &Database) -> Result<()> {
        let record = self.create_record()?;

        query!(r#"
            INSERT INTO package (base, added, source, srcinfo, pkgbuild, version, enabled, clean, schedule, prepare, flags, dependency)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        "#,
            record.base, record.added, record.source, record.srcinfo, record.pkgbuild, record.version, record.enabled, record.clean, record.schedule, record.prepare, record.flags, record.dependency
        )
            .execute(db).await?;

        Ok(())
    }

    /// Updates the settings inside the database
    pub async fn change_settings(&self, db: &Database) -> Result<()> {
        let record = self.create_record()?;

        query!(r#"
            UPDATE package
            SET enabled = $2, clean = $3, schedule = $4, prepare = $5, flags = $6
            WHERE base = $1
        "#,
            record.base, record.enabled, record.clean, record.schedule, record.prepare, record.flags
        )
            .execute(db).await?;

        Ok(())
    }

    /// Updates the sources inside the database
    pub async fn change_sources(&self, db: &Database) -> Result<()> {
        let record = self.create_record()?;

        query!(r#"
            UPDATE package
            SET source = $2, srcinfo = $3, pkgbuild = $4, version = $5
            WHERE base = $1
        "#,
            record.base, record.source, record.srcinfo, record.pkgbuild, record.version
        )
            .execute(db).await?;

        Ok(())
    }

    /// Deletes the package from the database
    pub async fn delete(&self, db: &Database) -> Result<()> {
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
