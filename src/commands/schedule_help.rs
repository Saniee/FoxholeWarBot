use crate::{Context, Error};

/// Lists the phrases that can be used for the `/schedule-report` schedule option.
///
/// Deliberately not gated on guild setup — it's documentation, and the old
/// version refused to explain the syntax until you'd configured a shard (QA L-5).
#[poise::command(slash_command, guild_only)]
pub async fn schedule_help(ctx: Context<'_>) -> Result<(), Error> {
    // Inline rather than the old link to a prnt.sc screenshot, which was one
    // link-rot away from being the only documentation (QA L-1).
    ctx.send(
        poise::CreateReply::default()
            .ephemeral(true)
            .content(
                "**Scheduling a report**\n\
                 Schedule names must be unique within this server.\n\n\
                 The `schedule` option accepts plain-English phrases, for example:\n\
                 • `every 30 minutes`\n\
                 • `every 2 hours`\n\
                 • `at 6:30 pm`\n\
                 • `every day at 09:00`\n\
                 • `on Monday at 5:00 pm`\n\
                 • `every Friday at 18:00`\n\n\
                 A plain 6-field cron expression (`sec min hour day month weekday`) also works, \
                 e.g. `0 0 12 * * *` for daily at noon.\n\n\
                 Times are interpreted in **UTC**.",
            ),
    )
    .await?;

    Ok(())
}
