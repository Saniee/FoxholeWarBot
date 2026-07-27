//! Turning a picked cadence into a cron expression, and a timezone name into a
//! zone. See `specs/schedule-input.md`.
//!
//! The rule this module exists to enforce: **the bot writes the cron string, the
//! user never does.** Everything a user can supply here is either a choice from
//! a fixed list, an `HH:MM`, or an IANA name that came out of an autocomplete —
//! inputs with one obvious spelling and an error message that names it. The one
//! free-text path left is [`Frequency::Custom`], and it has to prove itself by
//! showing real fire times before anything is stored.

use std::str::FromStr;

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use croner::Cron;
use thiserror::Error;
use tokio_cron_scheduler::Job;

/// No schedule may fire more often than this, whatever it renders.
///
/// Not a performance limit — a courtesy one. Every run is a fetch against
/// Clapfoot's War API and a post into someone's channel, and a report that
/// arrives every few minutes reads as a bot spamming a server rather than
/// reporting on a war. Half an hour is well inside how fast the front actually
/// moves, and `/get-map` is still on demand and unlimited for anyone who wants
/// a hex *now*.
pub const MIN_INTERVAL_MINUTES: i64 = 30;

/// A full-map schedule may not fire more often than this.
///
/// The approval queue (`specs/premium-full-map.md`) decides *whether* a
/// guild may schedule the world map, not how often — so without a floor, one
/// approval can turn into 53 regions fetched and composited every five minutes,
/// forever. An hour is generous next to a war that moves over days.
pub const FULL_MAP_MIN_INTERVAL_MINUTES: i64 = 60;

/// The floor that applies to a given target: the full map buys an hour, every
/// other schedule the global half hour.
pub fn min_interval_minutes(full_map: bool) -> i64 {
    if full_map {
        FULL_MAP_MIN_INTERVAL_MINUTES
    } else {
        MIN_INTERVAL_MINUTES
    }
}

/// How many fire times to show before saving a schedule.
pub const PREVIEW_COUNT: usize = 3;

#[derive(Debug, Error)]
pub enum ScheduleError {
    #[error("`{0}` isn't a time I understand. Use 24-hour `HH:MM`, for example `18:30`.")]
    BadTime(String),
    #[error("pick a `custom` expression, or choose one of the listed frequencies")]
    MissingCustom,
    #[error("`{0}` isn't a schedule the scheduler understands")]
    BadCustom(String),
    #[error("`{0}` isn't a timezone I know. Pick one from the autocomplete list.")]
    BadTimezone(String),
    #[error("that schedule has no future run times")]
    NeverFires,
}

/// How often a report posts. A fixed list, so the common path has nothing to
/// get wrong — plus one escape hatch for people who know exactly what they want.
#[derive(Debug, Clone, Copy, PartialEq, Eq, poise::ChoiceParameter)]
pub enum Frequency {
    // Nothing shorter than half an hour is offered — see `MIN_INTERVAL_MINUTES`.
    // Leaving "every 15 minutes" on the list and then refusing it would be a
    // worse command than not offering it.
    #[name = "Every 30 minutes"]
    Every30Minutes,
    #[name = "Hourly"]
    Hourly,
    #[name = "Every 2 hours"]
    Every2Hours,
    #[name = "Every 3 hours"]
    Every3Hours,
    #[name = "Every 4 hours"]
    Every4Hours,
    #[name = "Every 6 hours"]
    Every6Hours,
    #[name = "Every 8 hours"]
    Every8Hours,
    #[name = "Every 12 hours"]
    Every12Hours,
    #[name = "Daily"]
    Daily,
    #[name = "Weekly"]
    Weekly,
    #[name = "Custom… (cron or a plain-English phrase)"]
    Custom,
}

/// Weekly needs a day. It is inert for every other frequency, which is the one
/// untidy corner of keeping this to a single command — the alternative was
/// splitting `/schedule-report` into subcommands and duplicating the five
/// options they share.
#[derive(Debug, Clone, Copy, PartialEq, Eq, poise::ChoiceParameter)]
pub enum Day {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Day {
    /// Cron's day-of-week number. Sunday is 0, matching chrono and croner.
    /// Written as a number rather than `MON`, so nothing depends on how the
    /// parser feels about three-letter names.
    fn number(self) -> u32 {
        match self {
            Day::Sunday => 0,
            Day::Monday => 1,
            Day::Tuesday => 2,
            Day::Wednesday => 3,
            Day::Thursday => 4,
            Day::Friday => 5,
            Day::Saturday => 6,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Day::Monday => "Monday",
            Day::Tuesday => "Tuesday",
            Day::Wednesday => "Wednesday",
            Day::Thursday => "Thursday",
            Day::Friday => "Friday",
            Day::Saturday => "Saturday",
            Day::Sunday => "Sunday",
        }
    }
}

/// The two things a schedule needs stored: the expression the scheduler runs,
/// and the phrase a human reads in the embed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cadence {
    pub cron: String,
    pub label: String,
}

/// Builds the schedule from what the user picked.
///
/// Every branch but `Custom` is a total function of a choice and an optional
/// `HH:MM`: there is no phrase to fail to parse, so the only error a normal user
/// can hit is a malformed time, and that error carries an example.
pub fn cadence(
    frequency: Frequency,
    at_time: Option<&str>,
    day: Option<Day>,
    custom: Option<&str>,
) -> Result<Cadence, ScheduleError> {
    if frequency == Frequency::Custom {
        let raw = custom
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or(ScheduleError::MissingCustom)?;

        // Accepts both a 6-field cron expression and the English phrases the old
        // free-text box took — existing users have phrases that work, and there
        // is no reason to take that away from the people it works for.
        let cron = Job::schedule_to_cron(raw)
            .map_err(|_| ScheduleError::BadCustom(raw.to_string()))?;

        // Parsed here as well, with exactly the builder the scheduler uses, so a
        // string that survives `schedule_to_cron` but not the parser is caught
        // now rather than at the first tick that never comes.
        parse_cron(&cron)?;

        return Ok(Cadence {
            cron,
            label: raw.to_string(),
        });
    }

    // The anchor: what the cadence lines up with. Absent, it's the top of the
    // hour — which is what every schedule did before this existed.
    let (hour, minute) = match at_time {
        Some(raw) => parse_at_time(raw)?,
        None => (0, 0),
    };

    let cadence = match frequency {
        Frequency::Every30Minutes => sub_hourly(30, minute),
        Frequency::Hourly => Cadence {
            cron: format!("0 {minute} * * * *"),
            label: match minute {
                0 => "hourly, on the hour".to_string(),
                m => format!("hourly, at :{m:02}"),
            },
        },
        Frequency::Every2Hours => every_n_hours(2, hour, minute),
        Frequency::Every3Hours => every_n_hours(3, hour, minute),
        Frequency::Every4Hours => every_n_hours(4, hour, minute),
        Frequency::Every6Hours => every_n_hours(6, hour, minute),
        Frequency::Every8Hours => every_n_hours(8, hour, minute),
        Frequency::Every12Hours => every_n_hours(12, hour, minute),
        Frequency::Daily => Cadence {
            cron: format!("0 {minute} {hour} * * *"),
            label: format!("daily at {hour:02}:{minute:02}"),
        },
        Frequency::Weekly => {
            // No day picked is Monday rather than an error: the option is
            // optional for every other frequency, and refusing here would mean
            // the one required-but-not-really option in the command.
            let day = day.unwrap_or(Day::Monday);

            Cadence {
                cron: format!("0 {minute} {hour} * * {}", day.number()),
                label: format!("every {} at {hour:02}:{minute:02}", day.name()),
            }
        }
        // Handled above, before any of this.
        Frequency::Custom => unreachable!(),
    };

    Ok(cadence)
}

/// "Every 30 minutes at :07" means :07 and :37 — the offset is kept rather than
/// rounded away, because someone who asked for :07 asked for it.
///
/// The minutes are written out in full instead of as `7-59/30`. A list can't be
/// misread, by the parser or by whoever reads the row in a year.
fn sub_hourly(step: u32, minute: u32) -> Cadence {
    let start = minute % step;
    let minutes: Vec<u32> = (start..60).step_by(step as usize).collect();

    Cadence {
        cron: format!("0 {} * * * *", join(&minutes)),
        label: match start {
            0 => format!("every {step} minutes"),
            m => format!("every {step} minutes, at :{m:02}"),
        },
    }
}

/// The piece the old free-text box couldn't express at all: "every 6 hours at
/// 03:30" is 03:30, 09:30, 15:30, 21:30 — not "six hours from whenever you
/// pressed enter", and not 00:00/06:00/12:00/18:00 either.
fn every_n_hours(step: u32, hour: u32, minute: u32) -> Cadence {
    let start = hour % step;
    let hours: Vec<u32> = (start..24).step_by(step as usize).collect();

    let times = hours
        .iter()
        .map(|h| format!("{h:02}:{minute:02}"))
        .collect::<Vec<_>>()
        .join(", ");

    Cadence {
        cron: format!("0 {minute} {} * * *", join(&hours)),
        label: format!("every {step} hours — {times}"),
    }
}

fn join(values: &[u32]) -> String {
    values
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// 24-hour `HH:MM`. A dot is accepted as well as a colon because plenty of
/// Europe writes `18.30`, and rejecting it would teach nobody anything.
fn parse_at_time(raw: &str) -> Result<(u32, u32), ScheduleError> {
    let trimmed = raw.trim();
    let bad = || ScheduleError::BadTime(trimmed.to_string());

    let (hour, minute) = trimmed
        .split_once([':', '.'])
        .ok_or_else(bad)?;

    let hour: u32 = hour.trim().parse().map_err(|_| bad())?;
    let minute: u32 = minute.trim().parse().map_err(|_| bad())?;

    if hour > 23 || minute > 59 {
        return Err(bad());
    }

    Ok((hour, minute))
}

/// An IANA name, from the autocomplete or typed. Case-insensitive on the way in
/// so `europe/london` works, but what gets stored is always the canonical name.
pub fn timezone(name: &str) -> Result<Tz, ScheduleError> {
    let trimmed = name.trim();

    if let Ok(tz) = Tz::from_str(trimmed) {
        return Ok(tz);
    }

    chrono_tz::TZ_VARIANTS
        .iter()
        .find(|tz| tz.name().eq_ignore_ascii_case(trimmed))
        .copied()
        .ok_or_else(|| ScheduleError::BadTimezone(trimmed.to_string()))
}

/// The stored timezone, or UTC if the row somehow holds a name this build of
/// `chrono-tz` doesn't know. A schedule firing an hour out beats a schedule that
/// refuses to be restored at boot.
pub fn timezone_or_utc(name: &str) -> Tz {
    timezone(name).unwrap_or_else(|_| {
        log::warn!("unknown timezone '{name}', falling back to UTC");
        Tz::UTC
    })
}

fn parse_cron(expression: &str) -> Result<Cron, ScheduleError> {
    // The same three calls `Job::new_async_tz` makes. Parsing it any other way
    // would mean the preview describes a schedule the scheduler doesn't run.
    Cron::new(expression)
        .with_seconds_required()
        .with_dom_and_dow()
        .parse()
        .map_err(|_| ScheduleError::BadCustom(expression.to_string()))
}

/// The next `count` times this expression fires, in `tz`.
///
/// This is what makes the free-text path safe. Today's failure isn't only that a
/// phrase gets rejected — an *accepted* one can mean something the user didn't
/// intend, and three real timestamps catch that before anything is stored.
///
/// Computed against the real zone, so a preview that spans a DST transition
/// reads in wall-clock terms. The scheduler itself works from a fixed offset
/// snapshotted at registration, which is why the nightly rebuild in
/// `utils::cron` exists.
pub fn next_fires(
    expression: &str,
    tz: Tz,
    count: usize,
) -> Result<Vec<DateTime<Utc>>, ScheduleError> {
    let cron = parse_cron(expression)?;
    let now = Utc::now().with_timezone(&tz);

    let fires: Vec<DateTime<Utc>> = cron
        .iter_after(now)
        .take(count)
        .map(|at| at.with_timezone(&Utc))
        .collect();

    if fires.is_empty() {
        return Err(ScheduleError::NeverFires);
    }

    Ok(fires)
}

/// The shortest gap between consecutive fire times, in minutes.
///
/// Sampled from the preview rather than reasoned about from the expression:
/// `*/5` and a hand-written list of twelve minutes are the same problem, and
/// only one of them looks like it.
pub fn shortest_gap_minutes(fires: &[DateTime<Utc>]) -> Option<i64> {
    fires
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).num_minutes())
        .min()
}

/// The fire times as Discord timestamp markup, so each reader sees them in their
/// own timezone without the bot formatting anything.
pub fn preview_lines(fires: &[DateTime<Utc>]) -> String {
    fires
        .iter()
        .map(|at| format!("• <t:{0}:F> (<t:{0}:R>)", at.timestamp()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// How a stored schedule is described. Falls back to the raw `schedule` for rows
/// created before `schedule_label` existed.
pub fn describe(label: Option<&str>, schedule: &str, timezone: &str) -> String {
    format!("{} ({timezone})", label.unwrap_or(schedule))
}
