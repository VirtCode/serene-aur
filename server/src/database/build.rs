use std::str::FromStr;
use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::{query, query_as};
use serene_data::build::{BuildProgress, BuildState};
use crate::build::BuildSummary;
use crate::database::{Database, DatabaseConversion};
use anyhow::{anyhow, Result};
use crate::package::Package;
use crate::runner::RunStatus;

/// See server/migrations/20240210164401_build.sql
#[derive(Debug)]
struct BuildRecord {
    package: String,

    /// id
    started: NaiveDateTime,
    ended: Option<NaiveDateTime>,

    state: String,
    progress: Option<String>,
    fatal: Option<String>,

    version: Option<String>,

    run_success: Option<bool>,
    run_logs: Option<String>,
    run_started: Option<NaiveDateTime>,
    run_ended: Option<NaiveDateTime>,
}

impl DatabaseConversion<BuildRecord> for BuildSummary {
    fn create_record(&self) -> Result<BuildRecord> {

        let (state, progress, fatal) = match &self.state {
            BuildState::Running(p) => { ("running".to_owned(), Some(p.to_string()), None) }
            BuildState::Success => { ("success".to_owned(), None, None) }
            BuildState::Failure => { ("failure".to_owned(), None, None) }
            BuildState::Fatal(m, p) => { ("fatal".to_owned(), Some(p.to_string()), Some(m.clone())) }
        };

        Ok(BuildRecord {
            package: self.package.clone(),
            started: self.started.naive_utc(),

            ended: self.ended.map(|t| t.naive_utc()),
            state, progress, fatal,
            version: self.version.clone(),
            run_success: self.logs.as_ref().map(|s| s.success),
            run_logs: self.logs.as_ref().map(|s| s.logs.clone()),
            run_started: self.logs.as_ref().map(|s| s.started.naive_utc()),
            run_ended: self.logs.as_ref().map(|s| s.ended.naive_utc())
        })
    }

    fn from_record(other: BuildRecord) -> Result<Self> where Self: Sized {
        let state = match (other.state.as_str(), other.progress, other.fatal) {
            ("success", None, None) => { BuildState::Success },
            ("failure", None, None) => { BuildState::Failure },
            ("running", Some(p), None) => {
                BuildState::Running(BuildProgress::from_str(&p).map_err(|_| anyhow!("no correct progress"))?)
            }
            ("fatal", Some(p), Some(m)) => {
                BuildState::Fatal(m, BuildProgress::from_str(&p).map_err(|_| anyhow!("no correct progress"))?)
            }
            _ => return Err(anyhow!("no valid state representation found"))
        };

        Ok(BuildSummary {
            package: other.package,
            state,
            version: other.version,
            started: other.started.and_utc(),
            ended: other.ended.map(|d| d.and_utc()),
            logs: match (other.run_success, other.run_logs, other.run_started, other.run_ended) {
                (Some(success), Some(logs), Some(started), Some(ended)) => Some(RunStatus {
                    success, logs, started: started.and_utc(), ended: ended.and_utc()
                }),
                _ => None
            }
        })
    }
}

impl BuildSummary {
    pub async fn find(date: &DateTime<Utc>, base: &str, db: &Database) -> Result<Option<Self>> {
        let naive = date.naive_utc();

        let record = query_as!(BuildRecord, r#"
            SELECT * FROM build WHERE started = $1 AND package = $2
        "#,
            naive, base
        )
            .fetch_optional(db).await?;

        record.map(BuildSummary::from_record).transpose()
    }

    pub async fn find_all_for_package(base: &str, db: &Database) -> Result<Vec<Self>> {
        let records = query_as!(BuildRecord, r#"
            SELECT * FROM build WHERE package = $1 ORDER BY started DESC
        "#,
            base
        )
            .fetch_all(db).await?;

        records.into_iter().map(BuildSummary::from_record).collect()
    }

    pub async fn find_latest_for_package(base: &str, db: &Database) -> Result<Option<Self>> {
        let record = query_as!(BuildRecord, r#"
            SELECT * FROM build WHERE package = $1 ORDER BY started DESC LIMIT 1
        "#,
            base
        )
            .fetch_optional(db).await?;

        record.map(BuildSummary::from_record).transpose()
    }

    pub async fn find_latest_n_for_package(base: &str, n: u32, db: &Database) -> Result<Vec<Self>> {
        let record = query_as!(BuildRecord, r#"
            SELECT * FROM build WHERE package = $1 ORDER BY started DESC LIMIT $2
        "#,
            base, n
        )
            .fetch_all(db).await?;

        record.into_iter().map(BuildSummary::from_record).collect()
    }

    pub async fn save(&self, db: &Database) -> Result<()> {
        let record = self.create_record()?;

        query!(r#"
            INSERT INTO build (package, started, ended, state, progress, fatal, version, run_success, run_logs, run_started, run_ended)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#,
            record.package, record.started, record.ended, record.state, record.progress, record.fatal, record.version, record.run_success, record.run_logs, record.run_started, record.run_ended
        )
            .execute(db).await?;

        Ok(())
    }

    pub async fn change(&self, db: &Database) -> Result<()> {
        let record = self.create_record()?;

        query!(r#"
            UPDATE build
            SET ended = $2, state = $3, progress = $4, fatal = $5, version = $6, run_success = $7, run_logs = $8, run_started = $9, run_ended = $10
            WHERE started = $1
        "#,
            record.started, record.ended, record.state, record.progress, record.fatal, record.version, record.run_success, record.run_logs, record.run_started, record.run_ended
        )
            .execute(db).await?;

        Ok(())
    }

    pub async fn delete(&self, db: &Database) -> Result<()> {
        let base = self.started.naive_utc();

        query!(r#"
            DELETE FROM build WHERE started = $1
        "#,
            base
        )
            .execute(db).await?;

        Ok(())
    }
}