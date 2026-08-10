use poise::serenity_prelude as serenity;

use crate::commands::common::{
    autocomplete_schedule_target, autocomplete_timezone, defer_for, guild_settings,
    placeholder_submitted, FULL_MAP_TARGET,
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
#[allow(clippy::too_many_arguments)]
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
    #[description = "Only for Custom: a cron expression or a plain-English phrase."] custom: Option<
        String,
    >,
) -> Result<(), Error> {
    let Some(guild) = guild_settings(ctx).await? else {
        return Ok(());
    };

    defer_for(ctx, &guild).await?;

    // Worth more here than on the one-shot commands: accepting the placeholder
    // would write a row and register a cron job whose every firing renders a
    // region that doesn't exist, posting a failure to the channel on a timer
    // until somebody deletes it.
    if placeholder_submitted(ctx, &map_name).await? {
        return Ok(());
    }

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

    // The floor. Measured from the previewed times rather than read off the
    // expression, so `Custom…` is held to the same rule as the choice list
    // without anyone having to reason about what `*/5` expands to.
    //
    // The full map buys an hour on top: the approval queue decides *whether* a
    // guild may schedule the world map and says nothing about how often, so this
    // is all that stands between one approval and 53 regions re-rendered every
    // few minutes.
    let floor = schedule::min_interval_minutes(target.is_none());

    if let Some(gap) = schedule::shortest_gap_minutes(&fires).filter(|gap| *gap < floor) {
        let reason = if target.is_none() {
            "All 53 regions are re-rendered every time, and the war doesn't move that fast."
        } else {
            "Anything quicker posts faster than the front actually moves, and reads as spam in \
             the channel it lands in."
        };

        ctx.say(format!(
            "That schedule would post as often as every **{gap} minutes**, and the minimum is \
             **{floor}**. {reason} `/get-map` and `/full-map` are still on demand and unlimited."
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

    let (webhook, webhook_created) = match resolve_webhook(ctx, &report_channel).await {
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

    let webhook_url = match webhook.url() {
        Ok(url) => url,
        Err(err) => {
            if webhook_created {
                delete_webhook(ctx, &webhook).await;
            }
            return Err(err.into());
        }
    };

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
            if webhook_created {
                delete_webhook(ctx, &webhook).await;
            }
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
        if webhook_created {
            delete_webhook(ctx, &webhook).await;
        }
        log::warn!("could not persist '{schedule_name}': {err}");
        ctx.say("Couldn't save that schedule. Nothing was changed — please try again.")
            .await?;
        return Ok(());
    }

    // The "it's broken" report that isn't: a schedule waits for its next slot,
    // so creating an hourly report at :05 posts nothing for 55 minutes. Said
    // here, next to the timestamp that proves it, rather than in a help page
    // nobody reads until they're already annoyed.
    ctx.say(format!(
        "Scheduled report `{schedule_name}` created — **{}** ({}), posting to <#{}>.\n\
         **Nothing posts right now.** The first report arrives at the first time below \
         (<t:{}:R>) — a schedule waits for its next slot, it doesn't fire on creation.\n\
         Next three runs:\n{}",
        cadence.label,
        zone.name(),
        report_channel.id,
        fires[0].timestamp(),
        schedule::preview_lines(&fires[..schedule::PREVIEW_COUNT.min(fires.len())]),
    ))
    .await?;

    Ok(())
}

/// Reuses the bot's existing report webhook in a channel, or creates one.
async fn resolve_webhook(
    ctx: Context<'_>,
    channel: &serenity::GuildChannel,
) -> Result<(serenity::Webhook, bool), serenity::Error> {
    let http = ctx.serenity_context().http.clone();

    let existing = channel
        .webhooks(http.clone())
        .await?
        .into_iter()
        .find(|webhook| webhook.name.as_deref() == Some(WEBHOOK_NAME));

    match existing {
        Some(webhook) => Ok((webhook, false)),
        None => {
            let webhook = channel
                .create_webhook(http, serenity::CreateWebhook::new(WEBHOOK_NAME))
                .await?;
            Ok((webhook, true))
        }
    }
}

async fn delete_webhook(ctx: Context<'_>, webhook: &serenity::Webhook) {
    if let Err(err) = webhook.delete(ctx.serenity_context().http.clone()).await {
        log::warn!("could not roll back the newly created report webhook: {err}");
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
