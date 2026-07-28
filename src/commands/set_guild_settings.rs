use crate::commands::common::{autocomplete_timezone, guild_id};
use crate::utils::cache::refresh_maps;
use crate::utils::db::Shard;
use crate::utils::schedule;
use crate::{Context, Error};

/// The three live shards, offered as a Discord choice list.
///
/// This replaces the old free-text option with a static autocomplete: a value
/// that wasn't one of the three silently fell through to Able.
///
/// All three are always offered, even when one is down. A choice list is baked
/// into the command at registration and Discord serves it from its own copy, so
/// there is no way to withdraw an option for the hour a shard is offline —
/// which is why the check below happens here, at the moment of choosing, and
/// why every command downstream still has to cope with a shard that has gone
/// down since.
#[derive(Debug, Clone, Copy, poise::ChoiceParameter)]
pub enum ShardChoice {
    Able,
    Baker,
    Charlie,
}

impl From<ShardChoice> for Shard {
    fn from(choice: ShardChoice) -> Self {
        match choice {
            ShardChoice::Able => Shard::Able,
            ShardChoice::Baker => Shard::Baker,
            ShardChoice::Charlie => Shard::Charlie,
        }
    }
}

/// Sets this server's shard, output visibility, timezone, tint and frontline.
//
// Kept to one line on purpose: poise hands the doc comment to Discord as the
// command description, and Discord rejects anything over 100 characters at
// registration time. Anything worth saying at length goes in a plain comment
// like this one, which the macro never sees.
//
// The tint shades each hex on /full-map by whichever faction holds it; see
// `utils::request_processing::controlling_team`. The frontline traces the
// contested boundary on both /get-map and /full-map; see
// `specs/active/frontline.md`.
#[poise::command(
    slash_command,
    guild_only,
    default_member_permissions = "ADMINISTRATOR"
)]
pub async fn set_guild_settings(
    ctx: Context<'_>,
    #[description = "Which shard to get data from."] shard: ShardChoice,
    #[description = "True shows command output to everyone, false only to the caller."]
    show_messages: bool,
    // Optional so that changing shards doesn't silently reset it: left out, the
    // guild keeps whatever it already chose (see `Database::upsert_guild`).
    #[description = "Shade each hex on /full-map by who controls it. Leave blank to keep current."]
    faction_tint: Option<bool>,
    // Same rule again. Both maps, not just the full one — the contested hex is
    // exactly where a per-hex answer is least useful.
    #[description = "Draw the contested frontline on maps. Leave blank to keep current."]
    frontline: Option<bool>,
    // Same "leave it alone" rule, and it matters more here: silently resetting a
    // server to UTC would move every schedule that inherits from it.
    #[description = "Default timezone for scheduled reports. Leave blank to keep current."]
    #[autocomplete = "autocomplete_timezone"]
    timezone: Option<String>,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let guild_id = guild_id(ctx)?;
    let shard: Shard = shard.into();

    // Confirm the shard can actually serve this guild before committing it.
    //
    // This asks for the region list rather than `/worldconquest/war`, and it
    // does not use `?`. Both were bugs: the old check only understood HTTP
    // status codes, so a shard that never answered at all — the exact meaning
    // of "unreachable" — propagated its transport error to `on_error` and told
    // the user "Something went wrong running that command", naming neither the
    // shard nor the problem. And a host answering on `/war` proves only that
    // something is listening; the region list is what every other command in
    // the bot actually needs.
    //
    // The list it fetches is cached as a side effect, so the autocomplete on
    // `/get-map` works immediately after setup rather than at the next refresh.
    let health = refresh_maps(shard).await;

    if let Some(problem) = health.explain(shard) {
        log::warn!(
            "{} was not set up for shard {}: {health:?}",
            guild_id,
            shard.as_str()
        );
        ctx.say(format!("{problem}\n\nNothing was changed."))
            .await?;
        return Ok(());
    }

    // Validated before it is stored, so a typo can't sit in the row until the
    // next time a schedule tries to use it and quietly falls back to UTC.
    // The canonical name is what gets stored, whatever case was typed.
    let timezone = match timezone.as_deref().map(schedule::timezone).transpose() {
        Ok(zone) => zone.map(|zone| zone.name().to_string()),
        Err(err) => {
            ctx.say(err.to_string()).await?;
            return Ok(());
        }
    };

    // One upsert for both create and update. The old create path hardcoded the
    // visibility flag off, so `show-messages` was silently ignored the first
    // time a server ran this (QA B-1).
    let existed = ctx.data().db.get_guild(guild_id).await?.is_some();
    let guild = ctx
        .data()
        .db
        .upsert_guild(
            guild_id,
            shard,
            show_messages,
            faction_tint,
            frontline,
            timezone.as_deref(),
        )
        .await?;

    let visibility = if show_messages {
        "visible to everyone"
    } else {
        "only visible to whoever runs them"
    };

    // Reported from the stored row rather than the option, so an omitted
    // `faction-tint` says what the guild actually has, not "unchanged".
    let tint = if guild.full_map_faction_tint {
        "on"
    } else {
        "off"
    };
    let front = if guild.frontline { "on" } else { "off" };

    ctx.say(format!(
        "{} — shard **{}**, command output {visibility}, full-map faction tint **{tint}**, \
         frontline **{front}**, timezone **{}**.",
        if existed { "Updated" } else { "Created" },
        shard.as_str(),
        guild.timezone
    ))
    .await?;

    Ok(())
}
