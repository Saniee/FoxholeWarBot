use crate::{Context, Error};

/// Explains how scheduled reports are timed, and how timezones are handled.
///
/// Deliberately not gated on guild setup — it's documentation, and the old
/// version refused to explain the syntax until you'd configured a shard (QA L-5).
#[poise::command(slash_command, guild_only)]
pub async fn schedule_help(ctx: Context<'_>) -> Result<(), Error> {
    // This used to teach a syntax: the accepted English phrases for a free-text
    // box. That box is gone — `/schedule-report` asks for a frequency from a
    // list — so what's left to explain is what the frequencies *mean* and which
    // clock they're read in, which is where the confusion actually was.
    ctx.send(
        poise::CreateReply::default()
            .ephemeral(true)
            .content(
                "**Scheduling a report**\n\
                 Pick a `frequency` from the list — every 15 minutes through weekly. \
                 Schedule names must be unique within this server.\n\n\
                 **`at_time` is what the cadence lines up with**, as 24-hour `HH:MM`. \
                 It isn't only for daily reports:\n\
                 • `every 6 hours` at `03:30` → 03:30, 09:30, 15:30, 21:30\n\
                 • `every 15 minutes` at `00:07` → :07, :22, :37, :52\n\
                 • `daily` at `18:00` → 18:00, once a day\n\n\
                 Left blank, it means the top of the hour. Note that these are \
                 **clock times, not \"from now\"** — `every 6 hours` created at 09:20 next \
                 fires at 12:00, not 15:20.\n\n\
                 **Timezones.** `/set-guild-settings` sets this server's default; the \
                 `timezone` option on `/schedule-report` overrides it for one report. \
                 Both take an IANA name from the autocomplete (`Europe/Berlin`), so \
                 daylight saving is handled for you. A schedule keeps the timezone it was \
                 created with, even if the server default changes later.\n\n\
                 **`Custom…`** takes a 6-field cron expression (`sec min hour day month weekday`) \
                 or a plain-English phrase, for anything the list doesn't cover.\n\n\
                 Whichever you pick, the reply shows **the next three times it will fire** \
                 before the schedule is saved, in your own timezone. If those three lines \
                 aren't what you meant, nothing has been created yet.",
            ),
    )
    .await?;

    Ok(())
}
