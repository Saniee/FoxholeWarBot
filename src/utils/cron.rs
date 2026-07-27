//! Scheduled map reports. See `specs/scheduling.md`.

use std::sync::Arc;
use std::str::FromStr;

use poise::serenity_prelude as serenity;
use thiserror::Error;
use tokio_cron_scheduler::{Job, JobScheduler, JobSchedulerError};
use uuid::Uuid;

use super::cache::save_maps_cache;
use super::db::Database;
use super::entitlement::{self, FullMapScheduling};
use super::format_timestamp;
use super::map_render::{render_full_map, render_region};
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
    /// The region to render, or `None` for the whole world map. Only the
    /// full-map variant is gated (`utils::entitlement`).
    pub map_name: Option<String>,
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

    /// Enforces the retention window `docs/privacy.md` promises for denied and
    /// withdrawn full-map requests. Daily is plenty for a 90-day window.
    pub async fn start_request_purge_job(&self, db: Database) -> Result<(), JobSchedulerError> {
        let job = Job::new_async("0 30 3 * * *", move |_uuid, _lock| {
            let db = db.clone();

            Box::pin(async move {
                match db.purge_stale_full_map_requests().await {
                    Ok(0) => {}
                    Ok(n) => log::info!("purged {n} full-map requests past the retention window"),
                    Err(err) => log::warn!("could not purge old full-map requests: {err}"),
                }
            })
        })?;

        self.scheduler.add(job).await?;
        log::info!("started the full-map request retention job");

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

    let (rendered, target) = match &job.map_name {
        Some(map_name) => {
            let rendered = render_region(
                &guild.shard,
                &guild.shard_name,
                map_name,
                job.draw_text,
                RenderConfig::default(),
            )
            .await?;

            (rendered, format!("Region: {}", display_name(map_name)))
        }
        // The gate is re-read here, not trusted from when the schedule was
        // created: an approval that can't be taken back isn't a gate
        // (`specs/active/premium-full-map.md`).
        None => {
            if !entitlement::scheduling().is_allowed(&guild).await {
                return go_dormant(http, db, job, &webhook).await;
            }

            let rendered = render_full_map(
                &guild.shard,
                &guild.shard_name,
                job.draw_text,
                RenderConfig {
                    faction_tint: guild.full_map_faction_tint,
                    ..RenderConfig::default()
                },
            )
            .await?;

            (rendered, "Region: the whole world map".to_string())
        }
    };

    let file_name = match &job.map_name {
        Some(map_name) => format!("{map_name}.png"),
        None => "full-map.png".to_string(),
    };
    let mut description = format!(
        "{target}\nLast API Update: {}",
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

/// A full-map schedule whose approval was withdrawn: render nothing, say so
/// once, and leave the job exactly where it is.
///
/// Deleting it instead would mean a guild that gets re-approved has to notice
/// its schedule is gone and rebuild it from memory, which is a worse outcome for
/// a decision the owner might reverse the same day. Silence would be worse
/// again: a report that simply stops arriving is indistinguishable from the bot
/// being down, and the guild would have no idea there was anything to ask about.
///
/// Once, though. The notice is a courtesy; on a two-hourly schedule, repeating
/// it is just the bot pestering a channel about a decision nobody there can act
/// on.
async fn go_dormant(
    http: Arc<serenity::Http>,
    db: Database,
    job: &ReportJob,
    webhook: &serenity::Webhook,
) -> Result<(), TickError> {
    log::info!(
        "'{}' is a full-map schedule for an unapproved guild, skipping this tick",
        job.schedule_name
    );

    let already_told = db
        .get_job_entry(job.guild_row_id, &job.schedule_name)
        .await?
        .is_some_and(|row| row.dormant_notified);

    if already_told {
        return Ok(());
    }

    let embed = serenity::CreateEmbed::new()
        .title(format!("Scheduled Report Paused: {}", job.schedule_name))
        .color((255, 170, 0))
        .description(
            "This server's approval for **scheduled** full-map reports isn't active, so this \
             schedule is paused rather than deleted — it resumes on its own if approval comes \
             back.\n\nNothing else is affected: `/full-map` still renders on demand for \
             everyone, and single-region schedules are unchanged. Run \
             `/request-full-map-schedule` to apply.",
        )
        .footer(serenity::CreateEmbedFooter::new("Paused at"))
        .timestamp(serenity::Timestamp::now());

    webhook
        .execute(http, false, serenity::ExecuteWebhook::new().embed(embed))
        .await?;

    // Only after the notice actually went out, so a failed send is retried next
    // tick instead of being silently swallowed.
    db.set_dormant_notified(job.guild_row_id, &job.schedule_name, true)
        .await?;

    Ok(())
}
