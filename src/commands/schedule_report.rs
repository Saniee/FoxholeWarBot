use poise::serenity_prelude as serenity;

use crate::commands::common::{
    autocomplete_schedule_target, autocomplete_timezone, defer_for, guild_settings,
    FULL_MAP_TARGET,
};
use crate::utils::cron::ReportJob;
use crate::utils::db::NewJob;
use crate::utils::entitlement::{self, FullMapScheduling};
use crate::utils::schedule::{self, Day, Frequency};
use crate::{Context, Error};

const WEBHOOK_NAME: &str = "Scheduled Map Report Webhook";

/// Creates a recurring scheduled map report in a channel.
///
/// Gated on Manage Webhooks: the command creates and deletes webhooks, so that's
/// the permission that actually matches what it does (QA S-4).
#[poise::command(
    slash_command,
    guild_only,
    default_member_permissions = "MANAGE_WEBHOOKS"
)]
pub async fn schedule_report(
    ctx: Context<'_>,
    #[description = "Region to gather data from, or the whole world map."]
    #[autocomplete = "autocomplete_schedule_target"]
    map_name: String,
    #[description = "Name for this schedule. Must be unique within this server."]
    schedule_name: String,
    #[description = "Channel the reports are posted to."] report_channel: serenity::GuildChannel,
    #[description = "How often to post."] frequency: Frequency,
    #[description = "Draw map labels (city names etc.)."] draw_text: bool,
    // Everything below is optional, and Discord requires the required options
    // first — so this order is registration order, not importance.
    #[description = "Time to line the schedule up with, 24-hour HH:MM. Default 00:00."]
    at_time: Option<String>,
    #[description = "Timezone for the time above. Defaults to this server's."]
    #[autocomplete = "autocomplete_timezone"]
    timezone: Option<String>,
    #[description = "Which day, for a weekly report. Default Monday."] day: Option<Day>,
    #[description = "Only for Custom: a cron expression or a plain-English phrase."]
    custom: Option<String>,
) -> Result<(), Error> {
    let Some(guild) = guild_settings(ctx).await? else {
        return Ok(());
    };

    defer_for(ctx, &guild).await?;

    let data = ctx.data();

    // The sentinel means the world map. Everything past this point works in
    // terms of `target`, so the gate is asked exactly once, here, and the rest of
    // the command can't accidentally skip it.
    let target = (map_name != FULL_MAP_TARGET).then(|| map_name.clone());

    if target.is_none() && !entitlement::scheduling().is_allowed(&guild).await {
        ctx.say(unapproved_message(ctx, guild.id).await).await?;
        return Ok(());
    }

    // The guild's default unless this report overrides it. Resolved here and
    // stored on the row, so changing the server default later can't move a
    // schedule that already exists.
    let zone_name = timezone.as_deref().unwrap_or(&guild.timezone);
    let zone = match schedule::timezone(zone_name) {
        Ok(zone) => zone,
        Err(err) => {
            ctx.say(err.to_string()).await?;
            return Ok(());
        }
    };

    // The user picked from a list; this turns the pick into cron. The only error
    // an ordinary path can produce is a malformed `at_time`, and it says so with
    // an example.
    let cadence = match schedule::cadence(frequency, at_time.as_deref(), day, custom.as_deref()) {
        Ok(cadence) => cadence,
        Err(err) => {
            ctx.say(err.to_string()).await?;
            return Ok(());
        }
    };

    // Nothing is stored until these are on screen. An expression that parses can
    // still mean something the user didn't intend — `every 6 hours` is absolute
    // clock times, not six hours from now — and three real timestamps are what
    // catch that, for the choice list as much as for `Custom…`.
    let fires = match schedule::next_fires(&cadence.cron, zone, schedule::PREVIEW_COUNT + 1) {
        Ok(fires) => fires,
        Err(err) => {
            ctx.say(err.to_string()).await?;
            return Ok(());
        }
    };

    // The gate says whether a guild may schedule the world map; it says nothing
    // about how often, so this is the only thing standing between an approval
    // and 53 regions re-rendered every five minutes.
    let full_map_gap = target
        .is_none()
        .then(|| schedule::shortest_gap_minutes(&fires))
        .flatten();

    if let Some(gap) = full_map_gap.filter(|gap| *gap < schedule::FULL_MAP_MIN_INTERVAL_MINUTES) {
        ctx.say(format!(
            "A **full-map** schedule can't run more often than once an hour, and some runs of \
             that one would be only {gap} minutes apart. All 53 regions are re-rendered every \
             time, and the war doesn't move that fast. `/full-map` is still on demand, \
             unlimited, and needs no approval."
        ))
        .await?;
        return Ok(());
    }

    // Names are unique per guild now, so this only rejects a clash inside this
    // server (QA B-2). The database constraint is still the real guard against a
    // race; this check just produces a friendlier message.
    if data
        .db
        .get_job_entry(guild.id, &schedule_name)
        .await?
        .is_some()
    {
        ctx.say(format!(
            "This server already has a scheduled report named `{schedule_name}`. Pick another name."
        ))
        .await?;
        return Ok(());
    }

    let webhook = match resolve_webhook(ctx, &report_channel).await {
        Ok(webhook) => webhook,
        // Missing Manage Webhooks used to panic here (QA C-11).
        Err(err) => {
            log::warn!("could not set up a webhook in {}: {err}", report_channel.id);
            ctx.say(format!(
                "I couldn't create a webhook in <#{}>. Make sure I have **Manage Webhooks** there.",
                report_channel.id
            ))
            .await?;
            return Ok(());
        }
    };

    let webhook_url = webhook.url()?;

    let job = ReportJob {
        guild_row_id: guild.id,
        schedule_name: schedule_name.clone(),
        schedule: cadence.cron.clone(),
        schedule_label: Some(cadence.label.clone()),
        timezone: zone.name().to_string(),
        webhook_url: webhook_url.clone(),
        map_name: target.clone(),
        draw_text,
    };

    // Schedule first, persist second: a scheduler rejection must not leave a row
    // behind that restores into nothing on the next boot (QA B-3).
    let uuid = match data
        .cron
        .schedule(ctx.serenity_context().http.clone(), data.db.clone(), &job)
        .await
    {
        Ok(uuid) => uuid,
        Err(err) => {
            log::warn!("could not schedule '{schedule_name}': {err}");
            ctx.say("The scheduler rejected that schedule. Check `/schedule-help` and try again.")
                .await?;
            return Ok(());
        }
    };

    let persisted = data
        .db
        .add_job_entry(&NewJob {
            guild: guild.id,
            job_name: schedule_name.clone(),
            schedule: cadence.cron.clone(),
            schedule_label: Some(cadence.label.clone()),
            timezone: zone.name().to_string(),
            webhook_url,
            map_name: target,
            draw_text,
            job_id: uuid.to_string(),
        })
        .await;

    if let Err(err) = persisted {
        // Roll the scheduler back so we don't post reports nothing knows about.
        data.cron.unschedule(Some(&uuid.to_string())).await;
        log::warn!("could not persist '{schedule_name}': {err}");
        ctx.say("Couldn't save that schedule. Nothing was changed — please try again.")
            .await?;
        return Ok(());
    }

    ctx.say(format!(
        "Scheduled report `{schedule_name}` created — **{}** ({}), posting to <#{}>.\n\
         Next three runs:\n{}",
        cadence.label,
        zone.name(),
        report_channel.id,
        schedule::preview_lines(&fires[..schedule::PREVIEW_COUNT.min(fires.len())]),
    ))
    .await?;

    Ok(())
}

/// Reuses the bot's existing report webhook in a channel, or creates one.
async fn resolve_webhook(
    ctx: Context<'_>,
    channel: &serenity::GuildChannel,
) -> Result<serenity::Webhook, serenity::Error> {
    let http = ctx.serenity_context().http.clone();

    let existing = channel
        .webhooks(http.clone())
        .await?
        .into_iter()
        .find(|webhook| webhook.name.as_deref() == Some(WEBHOOK_NAME));

    match existing {
        Some(webhook) => Ok(webhook),
        None => {
            channel
                .create_webhook(http, serenity::CreateWebhook::new(WEBHOOK_NAME))
                .await
        }
    }
}

/// What to say to a server that asked to schedule a full map without approval.
///
/// Deliberately not a refusal. The spec's whole position is that this is a queue
/// and not a paywall, and "you can't do that" followed by nothing is exactly how
/// a queue gets mistaken for a price. So: name the free path that already works,
/// name the command that opens the form, and say plainly that no money is
/// involved.
async fn unapproved_message(ctx: Context<'_>, guild_row: i64) -> String {
    let pending = ctx
        .data()
        .db
        .pending_request_for_guild(guild_row)
        .await
        .ok()
        .flatten();

    if let Some(request) = pending {
        return format!(
            "Request **#{}** (filed <t:{}:R>) is still waiting for a decision — you'll hear back \
             in the channel it named. `/full-map` works on demand meanwhile.",
            request.id, request.created_at
        );
    }

    "Scheduling the **whole world map** needs approval first — run `/request-full-map-schedule`.\n\
     It's free. Approval is a queue, not a paid tier: a scheduled full map re-renders all 53 \
     regions on a timer. `/full-map` and single-region schedules are unaffected."
        .to_string()
}
