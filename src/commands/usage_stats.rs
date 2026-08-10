//! `/usage-stats` — how much the bot is actually being used, globally.
//!
//! Reviewer-gated and always ephemeral, like `/full-map-requests`: the numbers
//! are aggregated across every server the bot is in, and no single server's
//! channel is the place to publish them. See `specs/usage-stats.md`.

use std::collections::HashMap;

use poise::serenity_prelude as serenity;

use crate::utils::db::{UsageBucket, UsageOverview, UsagePeriod};
use crate::utils::review;
use crate::utils::usage::{SCHEDULED_FULL_MAP, SCHEDULED_REPORT, SCHEDULE_COMMANDS};
use crate::utils::usage_stats_render;
use crate::{Context, Error};

/// Days shown when the option is left out. A week is the unit people compare
/// against — "is this Tuesday like last Tuesday".
const DEFAULT_DAYS: u32 = 7;

/// Weeks shown when the option is left out. Four is a month of trend without
/// needing a scroll.
const DEFAULT_WEEKS: u32 = 4;

/// Caps, which exist for Discord rather than for the database.
///
/// An embed field value is 1024 characters. A row of this table is ~57, so 14
/// days plus a header sits around 880 and 8 weeks around 570 — both comfortably
/// inside it, and both still readable on a phone. The rendering truncates as a
/// backstop, but these are what stop it ever having to.
const MAX_DAYS: u32 = 14;
const MAX_WEEKS: u32 = 8;

/// Shows how much the bot is being used, by day and by week.
#[poise::command(
    slash_command,
    // Keeps it out of the picker for people `is_reviewer` would refuse anyway.
    // The real gate is below; this is only tidiness.
    default_member_permissions = "ADMINISTRATOR"
)]
pub async fn usage_stats(
    ctx: Context<'_>,
    #[description = "How many days to show (1-14, default 7)."]
    #[min = 1]
    #[max = 14]
    days: Option<u32>,
    #[description = "How many weeks to show (1-8, default 4)."]
    #[min = 1]
    #[max = 8]
    weeks: Option<u32>,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    if !review::is_reviewer(ctx.author().id) {
        ctx.say("Only someone in the bot's `REVIEWER_IDS` can read the usage stats.")
            .await?;
        return Ok(());
    }

    // Clamped as well as declared. `min`/`max` are enforced by Discord, which
    // means they are enforced by a client we don't control.
    let days = days.unwrap_or(DEFAULT_DAYS).clamp(1, MAX_DAYS);
    let weeks = weeks.unwrap_or(DEFAULT_WEEKS).clamp(1, MAX_WEEKS);

    let db = &ctx.data().db;
    let overview = db.usage_overview().await?;
    let daily = db.usage_buckets(UsagePeriod::Daily, days as i64).await?;
    let weekly = db.usage_buckets(UsagePeriod::Weekly, weeks as i64).await?;

    let daily_buckets = day_buckets(days);
    let weekly_buckets = week_buckets(weeks);
    let png =
        usage_stats_render::render(&overview, &daily, &daily_buckets, &weekly, &weekly_buckets)?;
    let file_name = "usage-stats.png";
    let embed = serenity::CreateEmbed::new()
        .title("Usage report - all servers")
        .color((90, 160, 220))
        .description(describe(&overview))
        .image(format!("attachment://{file_name}"))
        .footer(serenity::CreateEmbedFooter::new(
            "Counts are per server per day, kept for 90 days. Nothing here is per user.",
        ))
        .timestamp(serenity::Timestamp::now());

    ctx.send(
        poise::CreateReply::default()
            .ephemeral(true)
            .embed(embed)
            .attachment(serenity::CreateAttachment::bytes(png, file_name)),
    )
    .await?;

    Ok(())
}

/// The standing picture, above the tables.
///
/// Separate from them because it answers a different question: the tables say
/// what happened, this says what is set up. A schedule that exists and has not
/// fired yet is invisible to the tables and is exactly what someone asking "is
/// the schedule system used" wants to know about.
fn describe(overview: &UsageOverview) -> String {
    format!(
        "**Set up right now**\n\
         Servers configured: **{}** · approved for full-map schedules: **{}**\n\
         Live schedules: **{}** (**{}** full-map) across **{}** server(s)",
        overview.guilds,
        overview.approved_guilds,
        overview.schedules,
        overview.full_map_schedules,
        overview.scheduling_guilds,
    )
}

/// The columns, in order. `SCHEDULE_COMMANDS` and the two delivery names are
/// deliberately different columns: one is people setting schedules up, the other
/// is the reports those schedules produced, and a server with one schedule
/// posting hourly is a very different picture from twenty servers each setting
/// one up and never being posted to.
const COLUMNS: [(&str, usize); 5] = [
    ("get-map", 7),
    ("full-map", 8),
    ("reports", 7),
    ("sch-cmd", 7),
    ("other", 5),
];

/// Which column a recorded command name belongs in.
fn column_of(command: &str) -> usize {
    // Older rows were recorded from Poise's Rust-facing snake_case names.
    // Normalize them on read so the dashboard can classify existing data too.
    let normalized = command.replace('_', "-");
    match normalized.as_str() {
        "get-map" => 0,
        "full-map" => 1,
        SCHEDULED_REPORT | SCHEDULED_FULL_MAP => 2,
        _ if SCHEDULE_COMMANDS.contains(&normalized.as_str()) => 3,
        _ => 4,
    }
}

/// Discord's cap on an embed field's value.
const FIELD_LIMIT: usize = 1024;

/// A fixed-width table in a code block, which is the only alignment Discord
/// gives you: an embed renders proportional text everywhere else, so a table
/// built out of spaces outside a fence is not a table.
#[allow(dead_code)]
fn table(rows: &[UsageBucket], buckets: &[String]) -> String {
    // Pivot first: the query returns one row per (bucket, command), and every
    // bucket the caller asked for has to appear whether or not the query
    // returned anything for it.
    let mut counts: HashMap<&str, ([i64; COLUMNS.len()], i64)> = HashMap::new();

    for row in rows {
        let entry = counts.entry(row.bucket.as_str()).or_default();

        match &row.command {
            Some(command) => entry.0[column_of(command)] += row.uses,
            // The `GROUPING SETS` total row. Only its distinct-server count is
            // taken — its `uses` is the sum of the per-command rows, which are
            // already being added up above.
            None => entry.1 = row.servers,
        }
    }

    let mut out = String::from("```\n");

    out.push_str(&format!("{:<10}", "bucket"));
    for (name, width) in COLUMNS {
        out.push_str(&format!(" {name:>width$}"));
    }
    out.push_str(&format!(" {:>7}\n", "servers"));

    for bucket in buckets {
        let (columns, servers) = counts.get(bucket.as_str()).copied().unwrap_or_default();

        let mut line = format!("{bucket:<10}");
        for (value, (_, width)) in columns.iter().zip(COLUMNS) {
            line.push_str(&format!(" {value:>width$}"));
        }
        line.push_str(&format!(" {servers:>7}\n"));

        // A backstop, not the plan — MAX_DAYS and MAX_WEEKS are what keep this
        // from firing. Dropping whole rows rather than truncating the finished
        // string, because a cut that lands inside the closing fence turns the
        // rest of the embed into code and reads as a rendering bug rather than
        // as a table that stopped early. An over-long field is rejected by
        // Discord outright, so the alternative is the command failing.
        if out.len() + line.len() + "```".len() > FIELD_LIMIT {
            break;
        }

        out.push_str(&line);
    }

    out.push_str("```");

    out
}

/// The day labels the table must show, newest first, matching the `YYYY-MM-DD`
/// the query formats its buckets as.
fn day_buckets(days: u32) -> Vec<String> {
    let today = chrono::Utc::now().date_naive();

    (0..days as i64)
        .filter_map(|back| today.checked_sub_signed(chrono::Duration::days(back)))
        .map(|date| date.format("%Y-%m-%d").to_string())
        .collect()
}

/// The same, for weeks — labelled by the Monday each starts on, because that is
/// what `date_trunc('week', …)` returns.
fn week_buckets(weeks: u32) -> Vec<String> {
    use chrono::Datelike;

    let today = chrono::Utc::now().date_naive();
    let monday = today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64);

    (0..weeks as i64)
        .filter_map(|back| monday.checked_sub_signed(chrono::Duration::weeks(back)))
        .map(|date| date.format("%Y-%m-%d").to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(bucket: &str, command: Option<&str>, uses: i64, servers: i64) -> UsageBucket {
        UsageBucket {
            bucket: bucket.to_string(),
            command: command.map(str::to_string),
            uses,
            servers,
        }
    }

    /// The line for a bucket, without the fence or the header.
    fn line_for<'a>(table: &'a str, bucket: &str) -> Option<&'a str> {
        table.lines().find(|line| line.starts_with(bucket))
    }

    #[test]
    fn commands_land_in_their_columns() {
        let rows = vec![
            row("2026-07-29", Some("get-map"), 4, 2),
            row("2026-07-29", Some("full-map"), 1, 1),
            row("2026-07-29", Some("scheduled-report"), 6, 1),
            row("2026-07-29", Some("schedule-report"), 3, 1),
            row("2026-07-29", Some("war-state"), 2, 1),
            row("2026-07-29", None, 16, 3),
        ];

        let table = table(&rows, &["2026-07-29".to_string()]);
        let counts: Vec<&str> = line_for(&table, "2026-07-29")
            .expect("the bucket should be rendered")
            .split_whitespace()
            .skip(1)
            .collect();

        // get-map, full-map, reports, sch-cmd, other, servers.
        assert_eq!(counts, ["4", "1", "6", "3", "2", "3"]);
    }

    /// Both delivery kinds share the `reports` column, and both count.
    #[test]
    fn scheduled_deliveries_are_added_together() {
        let rows = vec![
            row("2026-07-29", Some("scheduled-report"), 6, 1),
            row("2026-07-29", Some("scheduled-full-map"), 2, 1),
            row("2026-07-29", None, 8, 1),
        ];

        let table = table(&rows, &["2026-07-29".to_string()]);
        let counts: Vec<&str> = line_for(&table, "2026-07-29")
            .unwrap()
            .split_whitespace()
            .skip(1)
            .collect();

        assert_eq!(counts[2], "8");
    }

    /// A subcommand is counted under its qualified name, which belongs to
    /// nothing else — so it must land in `other` rather than matching some
    /// bare `list` the classifier could mistake it for.
    #[test]
    fn qualified_subcommand_names_fall_through_to_other() {
        assert_eq!(column_of("full-map-requests list"), 4);
    }

    #[test]
    fn snake_case_command_names_use_their_dashboard_columns() {
        assert_eq!(column_of("get_map"), 0);
        assert_eq!(column_of("full_map"), 1);
        assert_eq!(column_of("schedule_report"), 3);
    }

    /// The distinct-server count is the total row's, never the sum of the
    /// per-command ones — one server running three commands is one server.
    #[test]
    fn servers_come_from_the_total_row() {
        let rows = vec![
            row("2026-07-29", Some("get-map"), 1, 1),
            row("2026-07-29", Some("full-map"), 1, 1),
            row("2026-07-29", Some("war-state"), 1, 1),
            row("2026-07-29", None, 3, 1),
        ];

        let table = table(&rows, &["2026-07-29".to_string()]);
        let counts: Vec<&str> = line_for(&table, "2026-07-29")
            .unwrap()
            .split_whitespace()
            .skip(1)
            .collect();

        assert_eq!(counts.last(), Some(&"1"));
    }

    /// A day nobody used the bot has no rows at all. It still has to appear, as
    /// zeroes — a table that silently skips Sunday reads as lost data.
    #[test]
    fn quiet_buckets_are_rendered_as_zeroes() {
        let rows = vec![row("2026-07-29", Some("get-map"), 4, 1)];
        let buckets = ["2026-07-29".to_string(), "2026-07-28".to_string()];

        let table = table(&rows, &buckets);
        let quiet: Vec<&str> = line_for(&table, "2026-07-28")
            .expect("a bucket with no usage should still be a row")
            .split_whitespace()
            .skip(1)
            .collect();

        assert_eq!(quiet, ["0", "0", "0", "0", "0", "0"]);
    }

    /// The caps should mean this never fires, so the check is that the backstop
    /// leaves a *valid* table behind when it does.
    #[test]
    fn an_oversized_table_still_closes_its_fence() {
        let buckets: Vec<String> = (1..=200).map(|n| format!("bucket-{n}")).collect();

        let table = table(&[], &buckets);

        assert!(table.len() <= FIELD_LIMIT);
        assert!(table.starts_with("```\n"));
        assert!(table.ends_with("```"));
    }

    #[test]
    fn the_default_span_is_a_full_week_ending_today() {
        let days = day_buckets(DEFAULT_DAYS);

        assert_eq!(days.len(), DEFAULT_DAYS as usize);
        assert_eq!(days[0], chrono::Utc::now().format("%Y-%m-%d").to_string());
    }

    /// Week labels are Mondays, because that is what `date_trunc('week', …)`
    /// returns — a label the query never produces matches nothing and renders
    /// every week as empty.
    #[test]
    fn week_buckets_are_mondays_seven_days_apart() {
        use chrono::Datelike;

        let weeks = week_buckets(4);
        assert_eq!(weeks.len(), 4);

        for label in &weeks {
            let date = chrono::NaiveDate::parse_from_str(label, "%Y-%m-%d").unwrap();
            assert_eq!(date.weekday(), chrono::Weekday::Mon);
        }

        let newest = chrono::NaiveDate::parse_from_str(&weeks[0], "%Y-%m-%d").unwrap();
        let next = chrono::NaiveDate::parse_from_str(&weeks[1], "%Y-%m-%d").unwrap();
        assert_eq!((newest - next).num_days(), 7);
    }
}
