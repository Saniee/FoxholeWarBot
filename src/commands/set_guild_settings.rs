use reqwest::StatusCode;

use crate::commands::common::guild_id;
use crate::utils::db::Shard;
use crate::{Context, Error};

/// The three live shards, offered as a Discord choice list.
///
/// This replaces the old free-text option with a static autocomplete: a value
/// that wasn't one of the three silently fell through to Able.
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

/// Sets this server's shard, output visibility, and full-map faction tint.
//
// Kept to one line on purpose: poise hands the doc comment to Discord as the
// command description, and Discord rejects anything over 100 characters at
// registration time. Anything worth saying at length goes in a plain comment
// like this one, which the macro never sees.
//
// The tint shades each hex on /full-map by whichever faction holds it; see
// `utils::request_processing::controlling_team`.
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
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let guild_id = guild_id(ctx)?;
    let shard: Shard = shard.into();

    // Confirm the shard is actually up before committing the guild to it.
    let response = reqwest::Client::new()
        .get(format!("{}/worldconquest/war", shard.api_url()))
        .send()
        .await?;

    match response.status() {
        StatusCode::OK => {}
        StatusCode::SERVICE_UNAVAILABLE => {
            ctx.say(format!(
                "Shard **{}** is currently unavailable, so it wasn't set. Try another, or try again later.",
                shard.as_str()
            ))
            .await?;
            return Ok(());
        }
        status => {
            log::warn!("shard {} returned {status} during setup", shard.as_str());
            ctx.say(format!(
                "The Foxhole API returned `{status}` for shard **{}**. Try again shortly.",
                shard.as_str()
            ))
            .await?;
            return Ok(());
        }
    }

    // One upsert for both create and update. The old create path hardcoded the
    // visibility flag off, so `show-messages` was silently ignored the first
    // time a server ran this (QA B-1).
    let existed = ctx.data().db.get_guild(guild_id).await?.is_some();
    let guild = ctx
        .data()
        .db
        .upsert_guild(guild_id, shard, show_messages, faction_tint)
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

    ctx.say(format!(
        "{} — shard **{}**, command output {visibility}, full-map faction tint **{tint}**.",
        if existed { "Updated" } else { "Created" },
        shard.as_str()
    ))
    .await?;

    Ok(())
}
