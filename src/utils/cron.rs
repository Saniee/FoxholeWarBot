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
use super::logging::{self, LogFiles};
use super::map_render::{render_full_map, render_region};
use super::request_processing::RenderConfig;
use super::regions::display_name;
use super::schedule;

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
    /// The expression the scheduler runs — generated from the user's choices
    /// (`utils::schedule`), or, on a row that predates that, the phrase they
    /// typed. Both are strings `schedule_to_cron` accepts.
    pub schedule: String,
    /// The cadence in words, for the embed. `None` on pre-`0005` rows.
    pub schedule_label: Option<String>,
    /// IANA name the expression is interpreted in.
    pub timezone: String,
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

    /// Refreshes the cached per-shard region lists, hourly.
    ///
    /// Daily was the recovery time for an outage, not the refresh rate for a
    /// region list. A shard that was down when the list was last fetched has
    /// nothing cached, every autocomplete for it is empty, and none of that
    /// mends itself until the next run — so a shard could come back and stay
    /// unusable for most of a day. Hourly is three requests an hour against an
    /// API with no published rate limit, and it caps that window at an hour.
    pub async fn start_map_update_job(&self) -> Result<(), JobSchedulerError> {
        let job = Job::new_async("0 0 * * * *", |_uuid, _lock| {
            Box::pin(async move {
                save_maps_cache().await;
                // Hourly now, so this goes to the verbose log rather than the
                // console. Anything actually wrong is warned about by
                // `save_maps_cache` itself and still shows.
                log::debug!("refreshed the cached region lists");
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

    /// Keeps the log directory to its retention window, daily.
    ///
    /// Rotation only decides what the files are called; without this the mounted
    /// volume grows for as long as the bot runs. Twenty minutes after the
    /// request purge, so the two aren't doing filesystem work at the same
    /// minute.
    pub async fn start_log_prune_job(&self, logs: LogFiles) -> Result<(), JobSchedulerError> {
        let job = Job::new_async("0 50 3 * * *", move |_uuid, _lock| {
            let logs = logs.clone();

            Box::pin(async move {
                logging::prune_and_report(&logs);
            })
        })?;

        self.scheduler.add(job).await?;
        log::info!("started the log retention job");

        Ok(())
    }

    /// Re-registers every stored schedule from its timezone, nightly.
    ///
    /// Not housekeeping — this is the only thing that makes stored timezones
    /// true. `Job::new_async_tz` reads the zone's offset **once**, at
    /// construction, and keeps that fixed offset for the life of the job: a
    /// schedule created in January fires an hour out all summer, and only a
    /// restart fixes it. Rebuilding every night means any transition is
    /// corrected within a day of itself, without anyone noticing there was
    /// something to correct.
    ///
    /// Deliberately not "rebuild only on the two transition dates per zone":
    /// that is more code, per zone, to save a few seconds of work once a day.
    pub async fn start_job_rebuild_job(
        &self,
        http: Arc<serenity::Http>,
        db: Database,
    ) -> Result<(), JobSchedulerError> {
        let handler = self.clone();

        // Late enough that it isn't competing with the daily cache refresh, and
        // an hour that is the middle of the night for nobody in particular.
        let job = Job::new_async("0 20 4 * * *", move |_uuid, _lock| {
            let handler = handler.clone();
            let http = http.clone();
            let db = db.clone();

            Box::pin(async move {
                handler.load_jobs(http, &db, true).await;
            })
        })?;

        self.scheduler.add(job).await?;
        log::info!("started the nightly schedule rebuild");

        Ok(())
    }

    /// Re-registers every stored schedule at startup.
    ///
    /// Each row is joined to its own guild, and a row that can't be restored is
    /// skipped rather than aborting the rest (QA C-7, C-8).
    pub async fn restore_jobs(&self, http: Arc<serenity::Http>, db: &Database) {
        self.load_jobs(http, db, false).await;
    }

    /// `replace` drops each row's currently registered job first. At startup
    /// there is nothing registered to drop — the stored UUID belongs to a
    /// previous process — so removing it would only log about ids the scheduler
    /// has never heard of. During a rebuild it is the whole point: without it,
    /// every guild would end up with two copies of every report.
    async fn load_jobs(&self, http: Arc<serenity::Http>, db: &Database, replace: bool) {
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
                schedule_label: row.schedule_label.clone(),
                timezone: row.timezone.clone(),
                webhook_url: row.webhook_url.clone(),
                map_name: row.map_name.clone(),
                draw_text: row.draw_text,
            };

            // Unscheduled before the replacement is registered, so a row can
            // never briefly have two live jobs posting the same report. The
            // cost is the other order's risk: if `schedule` then fails, this
            // report stops until the next rebuild or restart. That's the safer
            // way round — a schedule that fails to register here would have
            // failed at boot too, and a duplicated report is the failure users
            // actually notice.
            if replace {
                self.unschedule(row.job_id.as_deref()).await;
            }

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

        if replace {
            log::info!("rebuilt {restored}/{total} scheduled reports for their timezones");
        } else {
            log::info!("restored {restored}/{total} scheduled reports");
        }
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

        // An unknown name falls back to UTC rather than refusing the row: the
        // stored zone came out of an autocomplete over this same database, so
        // the only way to get here is a `chrono-tz` that no longer carries a
        // zone it used to, and a report an hour out beats a report that stops.
        let tz = schedule::timezone_or_utc(&job.timezone);

        let job = job.clone();
        let scheduled = Job::new_async_tz(cron.as_str(), tz, move |uuid, mut scheduler| {
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

    // The gate is re-read every tick, not trusted from when the schedule was
    // created: an approval that can't be taken back isn't a gate
    // (`specs/premium-full-map.md`). Checked before the placeholder, so a
    // dormant schedule posts the withdrawal notice and nothing else.
    if job.map_name.is_none() && !entitlement::scheduling().is_allowed(&guild).await {
        return go_dormant(http, db, job, &webhook).await;
    }

    let target = match &job.map_name {
        Some(map_name) => format!("Region: {}", display_name(map_name)),
        None => "Region: the whole world map".to_string(),
    };

    // Something in the channel before the work starts. A full map is 53 fetches
    // and a composite — long enough that a channel watching for a report sees
    // nothing at the time it was promised — and even a region report otherwise
    // just materialises with no warning. This is edited into the finished report
    // rather than left beside it, so the channel still gets one message per run.
    let placeholder = post_placeholder(http.clone(), &webhook, job, &target).await;

    let rendered = match &job.map_name {
        Some(map_name) => {
            render_region(
                &guild.shard,
                &guild.shard_name,
                map_name,
                job.draw_text,
                RenderConfig {
                    frontline: guild.frontline,
                    ..RenderConfig::default()
                },
            )
            .await
        }
        None => {
            render_full_map(
                &guild.shard,
                &guild.shard_name,
                job.draw_text,
                RenderConfig {
                    faction_tint: guild.full_map_faction_tint,
                    frontline: guild.frontline,
                    ..RenderConfig::default()
                },
            )
            .await
        }
    };

    // A render that failed must not leave "fetching the latest war data" sitting
    // in the channel until the next tick. The placeholder becomes the failure
    // notice, and the error still propagates so the tick is logged as failed.
    let rendered = match rendered {
        Ok(rendered) => rendered,
        Err(err) => {
            if let Some(message) = placeholder {
                report_failure(http, &webhook, job, message).await;
            }
            return Err(err.into());
        }
    };

    let file_name = match &job.map_name {
        Some(map_name) => format!("{map_name}.png"),
        None => "full-map.png".to_string(),
    };
    let mut description = format!(
        "{target}\nSchedule: {}\nLast API Update: {}",
        schedule::describe(job.schedule_label.as_deref(), &job.schedule, &job.timezone),
        format_timestamp(rendered.last_updated)
    );

    // Discord's own timestamp markup, so every reader sees the next run in
    // *their* timezone with no work from us. It used to print a UTC wall clock,
    // which is correct and useless to anyone who doesn't think in UTC — and a
    // fair share of "it fires at the wrong time" was a right schedule described
    // in the wrong clock.
    if let Some(next) = next_tick {
        description.push_str(&format!(
            "\nNext Scheduled Update: <t:{0}:F> (<t:{0}:R>)",
            next.timestamp()
        ));
    }

    let embed = serenity::CreateEmbed::new()
        .title(format!("Scheduled Report: {}", job.schedule_name))
        .color((0, 255, 0))
        .description(description)
        .image(format!("attachment://{file_name}"))
        .footer(serenity::CreateEmbedFooter::new("Rendered at"))
        .timestamp(serenity::Timestamp::now());

    let attachment = serenity::CreateAttachment::bytes(rendered.png, file_name);

    match placeholder {
        // The placeholder becomes the report. One message per run, and the
        // channel watched it happen instead of waiting on nothing.
        Some(message) => {
            let edited = webhook
                .edit_message(
                    http.clone(),
                    message,
                    serenity::EditWebhookMessage::new()
                        .new_attachment(attachment.clone())
                        .embed(embed.clone()),
                )
                .await;

            // An edit can fail for reasons the render didn't — the message was
            // deleted, the token was rotated. Posting fresh is better than
            // dropping a report that has already been rendered; the stale
            // placeholder goes with it so the channel isn't left with a
            // "fetching" line above the finished map.
            if let Err(err) = edited {
                log::warn!(
                    "could not edit the placeholder for '{}', posting fresh: {err}",
                    job.schedule_name
                );

                webhook
                    .execute(
                        http.clone(),
                        false,
                        serenity::ExecuteWebhook::new()
                            .add_file(attachment)
                            .embed(embed),
                    )
                    .await?;

                let _ = webhook.delete_message(http, None, message).await;
            }
        }
        None => {
            webhook
                .execute(
                    http.clone(),
                    false,
                    serenity::ExecuteWebhook::new()
                        .add_file(attachment)
                        .embed(embed),
                )
                .await?;
        }
    }

    Ok(())
}

/// Posts the "working on it" message and returns its id, or `None` if it
/// couldn't be posted.
///
/// Deliberately best-effort: this is a courtesy, and a report that renders fine
/// must not be lost because the notice ahead of it failed to send. `wait` is
/// `true` because the id is the whole point — without it there is nothing to
/// edit into the report.
async fn post_placeholder(
    http: Arc<serenity::Http>,
    webhook: &serenity::Webhook,
    job: &ReportJob,
    target: &str,
) -> Option<serenity::MessageId> {
    // Named for what is actually slow. A full map is 53 region fetches and a
    // composite, so it says so — a channel that knows it's waiting on the world
    // map doesn't read ten seconds as a broken bot.
    let detail = match &job.map_name {
        Some(_) => "Fetching the latest war data and rendering the map…",
        None => "Fetching the latest war data for all 53 regions and building the world map. \
                 This takes a few seconds…",
    };

    let embed = serenity::CreateEmbed::new()
        .title(format!("Scheduled Report: {}", job.schedule_name))
        // Grey, so the finished report's green is the thing that reads as done.
        .color((150, 150, 150))
        .description(format!("{target}\n{detail}"))
        .footer(serenity::CreateEmbedFooter::new("Started at"))
        .timestamp(serenity::Timestamp::now());

    let posted = webhook
        .execute(http, true, serenity::ExecuteWebhook::new().embed(embed))
        .await;

    match posted {
        Ok(message) => message.map(|message| message.id),
        Err(err) => {
            log::warn!(
                "could not post the placeholder for '{}': {err}",
                job.schedule_name
            );
            None
        }
    }
}

/// Turns a placeholder into a failure notice.
///
/// The alternative is deleting it, which leaves a channel that saw "fetching…"
/// with no idea what became of it. Naming the schedule and saying it will try
/// again is the difference between a transient API failure and a bot that looks
/// like it silently stopped.
async fn report_failure(
    http: Arc<serenity::Http>,
    webhook: &serenity::Webhook,
    job: &ReportJob,
    message: serenity::MessageId,
) {
    let embed = serenity::CreateEmbed::new()
        .title(format!("Scheduled Report: {}", job.schedule_name))
        .color((220, 70, 70))
        .description(
            "Couldn't fetch the war data for this report — the Foxhole API didn't answer in \
             time, or the map couldn't be rendered. The schedule is untouched and the next run \
             will try again.",
        )
        .footer(serenity::CreateEmbedFooter::new("Failed at"))
        .timestamp(serenity::Timestamp::now());

    let edited = webhook
        .edit_message(
            http,
            message,
            serenity::EditWebhookMessage::new().embed(embed),
        )
        .await;

    if let Err(err) = edited {
        log::warn!(
            "could not report the failed tick for '{}': {err}",
            job.schedule_name
        );
    }
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

    // Says *withdrawn*, not "isn't active". This code is only reachable for a
    // schedule that exists, and a full-map schedule can only have been created
    // while the guild was approved — so reaching here means an approval that was
    // granted has since been taken back. "Isn't active" left a reader guessing
    // between that and never having been approved at all, which is the one thing
    // this notice exists to settle.
    let embed = serenity::CreateEmbed::new()
        .title(format!("Full-Map Approval Withdrawn: {}", job.schedule_name))
        .color((255, 170, 0))
        .description(
            "This server's approval to run **scheduled** full-map reports has been withdrawn, \
             so this schedule is paused rather than deleted — it resumes on its own if approval \
             is granted again.\n\nNothing else is affected: `/full-map` still renders the world \
             map on demand for everyone, and single-region schedules are unchanged. To ask for \
             it back, run `/request-full-map-schedule`.",
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
