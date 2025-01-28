use crate::build::BuildSummary;
use crate::database::{Database, DatabaseConversion};
use crate::runner::RunStatus;
use anyhow::{anyhow, Result};
use chrono::{DateTime, NaiveDateTime, Utc};
use serene_data::build::{BuildProgress, BuildReason, BuildState};
use sqlx::{query, query_as};
use std::str::FromStr;

const STATE_PENDING: &str = "pending";
const STATE_CANCELLED: &str = "cancelled";
const STATE_RUNNING: &str = "running";
const STATE_SUCCESS: &str = "success";
const STATE_FAILURE: &str = "failure";
const STATE_FATAL: &str = "fatal";

/// See migrations:
/// server/migrations/20240210164401_build.sql
/// server/migrations/20240917122808_build_reason.sql
#[derive(Debug)]
struct BuildRecord {
    package: String,

    /// id
    started: NaiveDateTime,
    ended: Option<NaiveDateTime>,

    reason: String,
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
            BuildState::Pending => (STATE_PENDING.to_owned(), None, None),
            BuildState::Cancelled(m) => (STATE_CANCELLED.to_owned(), None, Some(m.clone())),
            BuildState::Running(p) => (STATE_RUNNING.to_owned(), Some(p.to_string()), None),
            BuildState::Success => (STATE_SUCCESS.to_owned(), None, None),
            BuildState::Failure => (STATE_FAILURE.to_owned(), None, None),
            BuildState::Fatal(m, p) => {
                (STATE_FATAL.to_owned(), Some(p.to_string()), Some(m.clone()))
            }
        };

        Ok(BuildRecord {
            package: self.package.clone(),
            started: self.started.naive_utc(),

            ended: self.ended.map(|t| t.naive_utc()),
            state,
            progress,
            fatal,
            reason: self.reason.to_string(),
            version: self.version.clone(),
            run_success: self.logs.as_ref().map(|s| s.success),
            run_logs: self.logs.as_ref().map(|s| s.logs.clone()),
            run_started: self.logs.as_ref().map(|s| s.started.naive_utc()),
            run_ended: self.logs.as_ref().map(|s| s.ended.naive_utc()),
        })
    }

    fn from_record(other: BuildRecord) -> Result<Self>
    where
        Self: Sized,
    {
        let state = match (other.state.as_str(), other.progress, other.fatal) {
            (STATE_SUCCESS, None, None) => BuildState::Success,
            (STATE_FAILURE, None, None) => BuildState::Failure,
            (STATE_PENDING, None, None) => BuildState::Pending,
            (STATE_CANCELLED, None, Some(m)) => BuildState::Cancelled(m),
            (STATE_RUNNING, Some(p), None) => BuildState::Running(
                BuildProgress::from_str(&p).map_err(|_| anyhow!("no correct progress"))?,
            ),
            (STATE_FATAL, Some(p), Some(m)) => BuildState::Fatal(
                m,
                BuildProgress::from_str(&p).map_err(|_| anyhow!("no correct progress"))?,
            ),
            _ => return Err(anyhow!("no valid state representation found")),
        };

        Ok(BuildSummary {
            package: other.package,
            reason: BuildReason::from_str(&other.reason).unwrap_or(BuildReason::Unknown),
            state,
            version: other.version,
            started: other.started.and_utc(),
            ended: other.ended.map(|d| d.and_utc()),
            logs: match (other.run_success, other.run_logs, other.run_started, other.run_ended) {
                (Some(success), Some(logs), Some(started), Some(ended)) => Some(RunStatus {
                    success,
                    logs,
                    started: started.and_utc(),
                    ended: ended.and_utc(),
                }),
                _ => None,
            },
        })
    }
}

impl BuildSummary {
    pub async fn find(date: &DateTime<Utc>, base: &str, db: &Database) -> Result<Option<Self>> {
        let naive = date.naive_utc();

        let record = query_as!(
            BuildRecord,
            r#"
            SELECT * FROM build WHERE started = $1 AND package = $2
        "#,
            naive,
            base
        )
        .fetch_optional(db)
        .await?;

        record.map(BuildSummary::from_record).transpose()
    }

    pub async fn find_nth_for_package(n: u32, base: &str, db: &Database) -> Result<Option<Self>> {
        let record = query_as!(
            BuildRecord,
            r#"
            SELECT * FROM build WHERE package = $1 ORDER BY started ASC LIMIT $2, 1
        "#,
            base,
            n
        )
        .fetch_optional(db)
        .await?;

        record.map(BuildSummary::from_record).transpose()
    }

    pub async fn count_for_package(base: &str, db: &Database) -> Result<u32> {
        let count = query!(
            r#"
            SELECT COUNT(1) as count FROM build WHERE package = $1
        "#,
            base,
        )
        .fetch_one(db)
        .await?
        .count;

        Ok(count as u32)
    }

    pub async fn find_all_for_package(base: &str, db: &Database) -> Result<Vec<Self>> {
        let records = query_as!(
            BuildRecord,
            r#"
            SELECT * FROM build WHERE package = $1 ORDER BY started DESC
        "#,
            base
        )
        .fetch_all(db)
        .await?;

        records.into_iter().map(BuildSummary::from_record).collect()
    }

    pub async fn find_latest_for_package(base: &str, db: &Database) -> Result<Option<Self>> {
        let record = query_as!(
            BuildRecord,
            r#"
            SELECT * FROM build WHERE package = $1 ORDER BY started DESC LIMIT 1
        "#,
            base
        )
        .fetch_optional(db)
        .await?;

        record.map(BuildSummary::from_record).transpose()
    }

    pub async fn find_latest_n_for_package(base: &str, n: u32, db: &Database) -> Result<Vec<Self>> {
        let record = query_as!(
            BuildRecord,
            r#"
            SELECT * FROM build WHERE package = $1 ORDER BY started DESC LIMIT $2
        "#,
            base,
            n
        )
        .fetch_all(db)
        .await?;

        record.into_iter().map(BuildSummary::from_record).collect()
    }

    pub async fn find_active(db: &Database) -> Result<Vec<Self>> {
        let record = query_as!(
            BuildRecord,
            r#"
            SELECT * FROM build WHERE state = $1 OR state = $2
        "#,
            STATE_PENDING,
            STATE_RUNNING
        )
        .fetch_all(db)
        .await?;

        record.into_iter().map(BuildSummary::from_record).collect()
    }

    pub async fn save(&self, db: &Database) -> Result<()> {
        let record = self.create_record()?;

        query!(r#"
            INSERT INTO build (package, started, ended, state, progress, fatal, version, run_success, run_logs, run_started, run_ended, reason)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        "#,
            record.package, record.started, record.ended, record.state, record.progress, record.fatal, record.version, record.run_success, record.run_logs, record.run_started, record.run_ended, record.reason
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

        query!(
            r#"
            DELETE FROM build WHERE started = $1
        "#,
            base
        )
        .execute(db)
        .await?;

        Ok(())
    }
}
