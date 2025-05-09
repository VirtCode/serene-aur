use crate::database::{Database, DatabaseConversion};
use crate::package::source::legacy::LegacySource;
use crate::package::srcinfo::{SrcinfoGeneratorInstance, SrcinfoWrapper};
use crate::package::{Package, SOURCE_FOLDER};
use actix_web_lab::sse::Data;
use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use log::info;
use serde_json::Value;
use sqlx::{query, query_as};
use std::path::Path;
use std::str::FromStr;

/// See migrations:
/// server/migrations/20240210163236_package.sql,
/// server/migrations/20240306195926_makepkg_flags.sql
/// server/migrations/20241004212454_built_state.sql
/// server/migrations/20241007180807_remove_version.sql
/// server/migrations/20250418161813_private.sql
#[derive(Debug)]
struct PackageRecord {
    /// id
    base: String,

    added: NaiveDateTime,
    source: String,
    srcinfo: Option<String>,
    pkgbuild: Option<String>,
    built_state: String,
    enabled: bool,
    private: bool,
    clean: bool,
    dependency: bool,
    schedule: Option<String>,
    prepare: Option<String>,
    flags: Option<String>,
}

impl DatabaseConversion<PackageRecord> for Package {
    fn create_record(&self) -> Result<PackageRecord> {
        Ok(PackageRecord {
            base: self.base.clone(),
            added: self.added.naive_utc(),
            source: serde_json::to_string(&self.source).context("failed to serialize source")?,
            srcinfo: self.srcinfo.as_ref().map(|s| s.to_string()),
            pkgbuild: self.pkgbuild.clone(),
            built_state: self.built_state.clone(),
            enabled: self.enabled,
            clean: self.clean,
            private: self.private,
            schedule: self.schedule.clone(),
            prepare: self.prepare.clone(),
            flags: if !self.flags.is_empty() {
                Some(serde_json::to_string(&self.flags).context("failed to serialize flags")?)
            } else {
                None
            },
            dependency: self.dependency,
        })
    }

    fn from_record(value: PackageRecord) -> Result<Package>
    where
        Self: Sized,
    {
        Ok(Self {
            base: value.base,
            added: value.added.and_utc(),
            source: serde_json::from_str(&value.source).context("failed to deserialize source")?,
            pkgbuild: value.pkgbuild,
            srcinfo: value.srcinfo.map(|a| SrcinfoWrapper::from_str(&a)).transpose()?,
            built_state: value.built_state,
            enabled: value.enabled,
            clean: value.clean,
            private: value.private,
            schedule: value.schedule,
            prepare: value.prepare,
            flags: value
                .flags
                .map(|s| serde_json::from_str(&s).context("failed to deserialize source"))
                .unwrap_or_else(|| Ok(vec![]))?,
            dependency: value.dependency,
        })
    }
}

impl Package {
    /// Returns whether the database contains a specific package
    pub async fn has(base: &str, db: &Database) -> Result<bool> {
        let amount = query!(
            r#"
            SELECT COUNT(base) as count FROM package WHERE base == $1
        "#,
            base
        )
        .fetch_one(db)
        .await?
        .count;

        Ok(amount > 0)
    }

    /// Find a specific package from the database
    pub async fn find(base: &str, db: &Database) -> Result<Option<Self>> {
        let record = query_as!(
            PackageRecord,
            r#"
            SELECT * FROM package WHERE base = $1
        "#,
            base
        )
        .fetch_optional(db)
        .await?;

        record.map(Package::from_record).transpose()
    }

    /// Find all packages from the database
    pub async fn find_all(db: &Database) -> Result<Vec<Self>> {
        let records = query_as!(
            PackageRecord,
            r#"
            SELECT * FROM package
        "#
        )
        .fetch_all(db)
        .await?;

        records.into_iter().map(Package::from_record).collect()
    }

    /// Find all packages from the database which were freshly migrated to built
    /// states
    pub async fn find_migrated_built_state(db: &Database) -> Result<Vec<Self>> {
        let records = query_as!(
            PackageRecord,
            r#"
            SELECT * FROM package WHERE built_state == $1
        "#,
            "migrated"
        )
        .fetch_all(db)
        .await?;

        records.into_iter().map(Package::from_record).collect()
    }

    /// Saves the package to the database for a first time
    pub async fn save(&self, db: &Database) -> Result<()> {
        let record = self.create_record()?;

        query!(r#"
            INSERT INTO package (base, added, source, srcinfo, pkgbuild, enabled, clean, private, schedule, prepare, flags, dependency, built_state)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        "#,
            record.base, record.added, record.source, record.srcinfo, record.pkgbuild, record.enabled, record.clean, record.private, record.schedule, record.prepare, record.flags, record.dependency, record.built_state
        )
            .execute(db).await?;

        Ok(())
    }

    /// Updates the settings inside the database
    pub async fn change_settings(&self, db: &Database) -> Result<()> {
        let record = self.create_record()?;

        query!(
            r#"
            UPDATE package
            SET enabled = $2, clean = $3, private = $4, schedule = $5, prepare = $6, flags = $7, dependency = $8
            WHERE base = $1
        "#,
            record.base,
            record.enabled,
            record.clean,
            record.private,
            record.schedule,
            record.prepare,
            record.flags,
            record.dependency
        )
        .execute(db)
        .await?;

        Ok(())
    }

    /// Updates the sources inside the database
    pub async fn change_sources(&self, db: &Database) -> Result<()> {
        let record = self.create_record()?;

        query!(
            r#"
            UPDATE package
            SET source = $2, srcinfo = $3, pkgbuild = $4, built_state = $5
            WHERE base = $1
        "#,
            record.base,
            record.source,
            record.srcinfo,
            record.pkgbuild,
            record.built_state
        )
        .execute(db)
        .await?;

        Ok(())
    }

    /// Deletes the package from the database
    pub async fn delete(&self, db: &Database) -> Result<()> {
        let base = &self.base;

        query!(
            r#"
            DELETE FROM package WHERE base = $1
        "#,
            base
        )
        .execute(db)
        .await?;

        Ok(())
    }
}

pub async fn migrate_sources(
    db: &Database,
    srcinfo_generator: &SrcinfoGeneratorInstance,
) -> Result<()> {
    let records = query_as!(
        PackageRecord,
        r#"
            SELECT * FROM package
        "#
    )
    .fetch_all(db)
    .await?;

    for mut record in records {
        let source: Value =
            serde_json::from_str(&record.source).context("source had invalid json")?;

        // a source is old if it has the key "type"
        if source["type"] != Value::Null {
            info!("migrating source of {} to new implementation", &record.base);

            let legacy: LegacySource = serde_json::from_value(source)
                .context("source has 'type' key but is not legacy")?;

            let legacy_state = legacy.get_state();

            let folder = Path::new(SOURCE_FOLDER).join(&record.base);
            let modern = legacy.migrate(&folder).await.context("failed to migrate source")?;

            // update source and create package
            record.source = serde_json::to_string(&modern)?;
            let mut package = Package::from_record(record)?;

            // package was up-to-date before, so also update the built_state
            if package.built_state == legacy_state {
                package.built_state = package.source.get_state();
            }

            // update the source to generate srcinfos if required and save to db
            package.update(srcinfo_generator).await?;
            package.change_sources(db).await?;
        }
    }

    Ok(())
}
