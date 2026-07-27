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

/// Sets the shard this server pulls data from, and whether command output is
/// visible to everyone.
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
    ctx.data()
        .db
        .upsert_guild(guild_id, shard, show_messages)
        .await?;

    let visibility = if show_messages {
        "visible to everyone"
    } else {
        "only visible to whoever runs them"
    };

    ctx.say(format!(
        "{} — shard **{}**, command output {visibility}.",
        if existed { "Updated" } else { "Created" },
        shard.as_str()
    ))
    .await?;

    Ok(())
}
