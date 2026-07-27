use poise::serenity_prelude as serenity;
use tokio_cron_scheduler::Job;

use crate::commands::common::{autocomplete_map, defer_for, guild_settings};
use crate::utils::cron::ReportJob;
use crate::utils::db::NewJob;
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
    #[description = "Region to gather data from."]
    #[autocomplete = "autocomplete_map"]
    map_name: String,
    #[description = "Name for this schedule. Must be unique within this server."]
    schedule_name: String,
    #[description = "Channel the reports are posted to."] report_channel: serenity::GuildChannel,
    #[description = "When to post. See /schedule-help for accepted phrases."] schedule: String,
    #[description = "Draw map labels (city names etc.)."] draw_text: bool,
) -> Result<(), Error> {
    let Some(guild) = guild_settings(ctx).await? else {
        return Ok(());
    };

    defer_for(ctx, &guild).await?;

    let data = ctx.data();

    if Job::schedule_to_cron(&schedule).is_err() {
        ctx.say(
            "That schedule isn't a phrase the scheduler understands. \
             Run `/schedule-help` to see what works.",
        )
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
        schedule: schedule.clone(),
        webhook_url: webhook_url.clone(),
        // Always a region here: `/schedule-report` cannot target the full map.
        // That path is `/request-full-map-schedule` and only opens once the
        // request is approved (`specs/active/premium-full-map.md`).
        map_name: Some(map_name.clone()),
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
            schedule,
            webhook_url,
            map_name: Some(map_name),
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
        "Scheduled report `{schedule_name}` created. Reports will appear in <#{}>.",
        report_channel.id
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
