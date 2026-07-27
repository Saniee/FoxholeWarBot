//! Scheduled map reports. See `specs/scheduling.md`.

use std::sync::Arc;
use std::str::FromStr;

use poise::serenity_prelude as serenity;
use thiserror::Error;
use tokio_cron_scheduler::{Job, JobScheduler, JobSchedulerError};
use uuid::Uuid;

use super::cache::save_maps_cache;
use super::db::Database;
use super::format_timestamp;
use super::map_render::render_region;
use super::request_processing::RenderConfig;
use super::regions::display_name;

#[derive(Debug, Error)]
pub enum CronError {
    #[error("the schedule '{0}' isn't a phrase the scheduler understands")]
    BadSchedule(String),
    #[error("the scheduler rejected the job: {0}")]
    Scheduler(#[from] JobSchedulerError),
}

/// Everything a tick needs. Carries the owning guild explicitly so a job can
/// never render another guild's shard (QA C-7).
#[derive(Debug, Clone)]
pub struct ReportJob {
    /// `guilds.id`, the surrogate row id.
    pub guild_row_id: i64,
    pub schedule_name: String,
    /// The user's original phrase, stored as-is so it stays readable.
    pub schedule: String,
    pub webhook_url: String,
    pub map_name: String,
    pub draw_text: bool,
}

#[derive(Clone)]
pub struct CronHandler {
    scheduler: JobScheduler,
}

impl CronHandler {
    pub async fn new() -> Result<Self, JobSchedulerError> {
        let scheduler = JobScheduler::new().await?;
        scheduler.start().await?;

        Ok(CronHandler { scheduler })
    }

    /// Refreshes the cached per-shard region lists once a day.
    pub async fn start_map_update_job(&self) -> Result<(), JobSchedulerError> {
        let job = Job::new_async("0 0 0 * * *", |_uuid, _lock| {
            Box::pin(async move {
                save_maps_cache().await;
                log::info!("refreshed the cached region lists");
            })
        })?;

        self.scheduler.add(job).await?;
        log::info!("started the region-list refresh job");

        Ok(())
    }

    /// Re-registers every stored schedule at startup.
    ///
    /// Each row is joined to its own guild, and a row that can't be restored is
    /// skipped rather than aborting the rest (QA C-7, C-8).
    pub async fn restore_jobs(&self, http: Arc<serenity::Http>, db: &Database) {
        let rows = match db.all_jobs_with_guilds().await {
            Ok(rows) => rows,
            Err(err) => return log::error!("could not read scheduled reports: {err}"),
        };

        if rows.is_empty() {
            return log::info!("no scheduled reports to restore");
        }

        let total = rows.len();
        let mut restored = 0;

        for row in rows {
            let job = ReportJob {
                guild_row_id: row.guild_row_id,
                schedule_name: row.job_name.clone(),
                schedule: row.schedule.clone(),
                webhook_url: row.webhook_url.clone(),
                map_name: row.map_name.clone(),
                draw_text: row.draw_text,
            };

            match self.schedule(http.clone(), db.clone(), &job).await {
                Ok(uuid) => {
                    // The scheduler hands out a fresh UUID each process, so the
                    // stored one is only ever valid for the current run.
                    if let Err(err) = db.update_job_uuid(row.job_row_id, &uuid.to_string()).await {
                        log::warn!(
                            "restored '{}' but could not store its new id: {err}",
                            row.job_name
                        );
                    }
                    restored += 1;
                }
                Err(err) => log::warn!(
                    "skipping scheduled report '{}' in guild {}: {err}",
                    row.job_name,
                    row.guild_id
                ),
            }
        }

        log::info!("restored {restored}/{total} scheduled reports");
    }

    /// Registers a job with the scheduler. Does **not** touch the database —
    /// callers persist only after this succeeds, so a rejected schedule can't
    /// leave an orphan row behind (QA B-3).
    pub async fn schedule(
        &self,
        http: Arc<serenity::Http>,
        db: Database,
        job: &ReportJob,
    ) -> Result<Uuid, CronError> {
        let cron = Job::schedule_to_cron(&job.schedule)
            .map_err(|_| CronError::BadSchedule(job.schedule.clone()))?;

        let job = job.clone();
        let scheduled = Job::new_async(cron.as_str(), move |uuid, mut scheduler| {
            let http = http.clone();
            let db = db.clone();
            let job = job.clone();

            Box::pin(async move {
                let next_tick = scheduler.next_tick_for_job(uuid).await.ok().flatten();

                if let Err(err) = run_report(http, db, &job, next_tick).await {
                    // A failed tick must never take the scheduler down with it.
                    log::warn!(
                        "scheduled report '{}' failed this tick: {err}",
                        job.schedule_name
                    );
                }
            })
        })?;

        Ok(self.scheduler.add(scheduled).await?)
    }

    /// Removes a job from the scheduler. An id we no longer recognize is not an
    /// error — the job is gone either way.
    pub async fn unschedule(&self, job_id: Option<&str>) {
        let Some(uuid) = job_id.and_then(|id| Uuid::from_str(id).ok()) else {
            return;
        };

        if let Err(err) = self.scheduler.remove(&uuid).await {
            log::warn!("could not remove job {uuid} from the scheduler: {err}");
        }
    }
}

type TickError = Box<dyn std::error::Error + Send + Sync>;

async fn run_report(
    http: Arc<serenity::Http>,
    db: Database,
    job: &ReportJob,
    next_tick: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<(), TickError> {
    // Re-read the guild every tick so a shard or visibility change is picked up
    // without a restart.
    let Some(guild) = db.get_guild_by_row(job.guild_row_id).await? else {
        log::warn!(
            "scheduled report '{}' has no owning guild any more, skipping",
            job.schedule_name
        );
        return Ok(());
    };

    let webhook = serenity::Webhook::from_url(http.clone(), &job.webhook_url).await?;

    let rendered = render_region(
        &guild.shard,
        &guild.shard_name,
        &job.map_name,
        job.draw_text,
        RenderConfig::default(),
    )
    .await?;

    let file_name = format!("{}.png", job.map_name);
    let mut description = format!(
        "Region: {}\nLast API Update: {}",
        display_name(&job.map_name),
        format_timestamp(rendered.last_updated)
    );

    // Scheduler ticks are UTC, so the embed says so rather than rendering them
    // in the host's local time (QA L-2).
    if let Some(next) = next_tick {
        description.push_str(&format!(
            "\nNext Scheduled Update: {} UTC",
            next.format("%Y %m %d %H:%M:%S")
        ));
    }

    let embed = serenity::CreateEmbed::new()
        .title(format!("Scheduled Report: {}", job.schedule_name))
        .color((0, 255, 0))
        .description(description)
        .image(format!("attachment://{file_name}"))
        .footer(serenity::CreateEmbedFooter::new("Rendered at"))
        .timestamp(serenity::Timestamp::now());

    webhook
        .execute(
            http.clone(),
            false,
            serenity::ExecuteWebhook::new()
                .add_file(serenity::CreateAttachment::bytes(rendered.png, file_name))
                .embed(embed),
        )
        .await?;

    Ok(())
}
