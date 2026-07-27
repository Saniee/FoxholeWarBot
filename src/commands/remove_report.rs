use poise::serenity_prelude as serenity;

use crate::commands::common::{defer_for, guild_id, guild_settings};
use crate::{Context, Error};

const MAX_CHOICES: usize = 25;

/// Removes a scheduled report.
///
/// Same Manage Webhooks gate as creating one (QA S-4).
#[poise::command(
    slash_command,
    guild_only,
    default_member_permissions = "MANAGE_WEBHOOKS"
)]
pub async fn remove_report(
    ctx: Context<'_>,
    #[description = "The schedule to delete."]
    #[autocomplete = "autocomplete_schedule"]
    schedule_name: String,
) -> Result<(), Error> {
    let Some(guild) = guild_settings(ctx).await? else {
        return Ok(());
    };

    defer_for(ctx, &guild).await?;

    let data = ctx.data();

    let Some(job) = data.db.get_job_entry(guild.id, &schedule_name).await? else {
        ctx.say(format!(
            "This server has no scheduled report named `{schedule_name}`. \
             Check the name and try again."
        ))
        .await?;
        return Ok(());
    };

    data.cron.unschedule(job.job_id.as_deref()).await;

    // Only tear the webhook down once nothing else is using it.
    let remaining = data
        .db
        .get_jobs_for_guild(guild.id)
        .await?
        .into_iter()
        .filter(|other| other.id != job.id && other.webhook_url == job.webhook_url)
        .count();

    if remaining == 0 {
        delete_webhook(ctx, &job.webhook_url).await;
    }

    data.db.remove_job_entry(guild.id, &schedule_name).await?;

    ctx.say(format!("Scheduled report `{schedule_name}` was removed."))
        .await?;

    Ok(())
}

/// A webhook someone already deleted by hand is the state we wanted anyway —
/// log it and move on instead of panicking (QA C-9).
async fn delete_webhook(ctx: Context<'_>, url: &str) {
    let http = ctx.serenity_context().http.clone();

    let webhook = match serenity::Webhook::from_url(http.clone(), url).await {
        Ok(webhook) => webhook,
        Err(err) => return log::info!("report webhook is already gone: {err}"),
    };

    if let Err(err) = webhook.delete(http).await {
        log::warn!("could not delete the report webhook: {err}");
    }
}

async fn autocomplete_schedule(
    ctx: Context<'_>,
    partial: &str,
) -> Vec<serenity::AutocompleteChoice> {
    let Ok(id) = guild_id(ctx) else {
        return Vec::new();
    };

    let Ok(Some(guild)) = ctx.data().db.get_guild(id).await else {
        return Vec::new();
    };

    let Ok(jobs) = ctx.data().db.get_jobs_for_guild(guild.id).await else {
        return Vec::new();
    };

    if jobs.is_empty() {
        return vec![serenity::AutocompleteChoice::new(
            "This server has no scheduled reports.",
            "-",
        )];
    }

    // Lowercased on both sides — the old filter was case-sensitive, so typing
    // "daily" never matched a schedule named "Daily" (QA L-3).
    let filter = partial.trim().to_lowercase();

    jobs.into_iter()
        .filter(|job| job.job_name.to_lowercase().contains(&filter))
        .take(MAX_CHOICES)
        .map(|job| serenity::AutocompleteChoice::new(job.job_name.clone(), job.job_name))
        .collect()
}
