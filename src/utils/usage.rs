//! Usage counters: what gets counted, under what name, and the one place a
//! failed count is swallowed. See `specs/usage-stats.md`.
//!
//! Two things feed the same table. A slash command is counted from poise's
//! `pre_command` hook in `main.rs`, and a scheduled report is counted by the
//! tick that delivered it (`utils::cron`). They share a namespace, which is why
//! the synthetic names below are spelled the way they are.

use crate::utils::db::Database;
use crate::Context;

/// A scheduled single-region report that was actually delivered.
///
/// Not a slash command, and it can never collide with one: poise registers every
/// command in `commands::all()`, none of which is named this, and a command
/// added later would have to be called `scheduled-report` to clash. Counting
/// deliveries under a name in the same column as the commands is what lets one
/// table answer "how much work did the schedule system do" beside "how often did
/// anyone type `/get-map`".
pub const SCHEDULED_REPORT: &str = "scheduled-report";

/// A scheduled **full-map** report that was delivered. Split from
/// [`SCHEDULED_REPORT`] because the two cost wildly different amounts — one
/// region against 53 — and a single number that mixes them can't be read as
/// load.
pub const SCHEDULED_FULL_MAP: &str = "scheduled-full-map";

/// Commands whose subject is scheduling, grouped into one column by
/// `/usage-stats`. Delivery counts are not in here: this measures people
/// *setting schedules up*, which is a different question from how many reports
/// went out.
pub const SCHEDULE_COMMANDS: [&str; 4] = [
    "schedule-report",
    "remove-report",
    "schedule-help",
    "request-full-map-schedule",
];

/// Counts one invocation of a slash command.
///
/// Called from `pre_command`, so it counts **attempts**, not successes. That is
/// the honest measure of demand: a `/full-map` that failed because the API was
/// down is someone who wanted the full map, and dropping it would make the
/// numbers look healthiest on exactly the days the bot worked worst.
///
/// `qualified_name` rather than `name`, so `/full-map-requests list` doesn't
/// count as a command called `list` — and doesn't merge with the `list`
/// subcommand of anything added later.
pub async fn record_command(ctx: Context<'_>) {
    // Every command is `guild_only`, so this is unreachable in practice. It is
    // still not an unwrap: a counter is the last thing that should be able to
    // take a command down (QA C-4 is exactly this shape).
    let Some(guild_id) = ctx.guild_id() else {
        return;
    };

    record(
        &ctx.data().db,
        &ctx.command().qualified_name,
        guild_id.get() as i64,
    )
    .await;
}

/// Counts one thing, and never lets the counting matter.
///
/// A failed write is a `debug!`, not a `warn!`. The database being unreachable
/// is already being reported by whatever real work is failing at the same
/// moment; a second line per command about the statistics would be the loudest
/// thing in the log and the least useful, on exactly the sinks
/// `specs/architecture.md` keeps quiet on purpose.
pub async fn record(db: &Database, command: &str, guild_id: i64) {
    if let Err(err) = db.record_usage(command, guild_id).await {
        log::debug!("could not record usage of '{command}': {err}");
    }
}
