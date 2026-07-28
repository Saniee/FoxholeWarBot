use poise::serenity_prelude as serenity;

use crate::commands::common::{defer_for, guild_settings, unreachable_message};
use crate::utils::api_definitions::foxhole::War;
use crate::utils::format_timestamp;
use crate::utils::http;
use crate::{Context, Error};

/// Gets the global state of the war on this server's shard.
#[poise::command(slash_command, guild_only)]
pub async fn war_state(ctx: Context<'_>) -> Result<(), Error> {
    let Some(guild) = guild_settings(ctx).await? else {
        return Ok(());
    };

    defer_for(ctx, &guild).await?;

    // `/war-state` is the command a user reaches for *because* something looks
    // wrong, so it is the one that must not answer "Something went wrong". Each
    // of these three was a `?`, and a shard that had gone offline since setup
    // failed at the first of them with no mention of the shard at all.
    let response = match http::client()
        .get(format!("{}/worldconquest/war", guild.shard))
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            log::warn!("could not reach shard {} for the war state: {err}", guild.shard_name);
            ctx.say(unreachable_message(&guild.shard_name)).await?;
            return Ok(());
        }
    };

    let status = response.status();

    if !status.is_success() {
        log::warn!("shard {} returned {status} for the war state", guild.shard_name);
        ctx.say(format!(
            "The Foxhole API returned `{status}` for shard **{}**. It may be between wars or \
             down — try again shortly.",
            guild.shard_name
        ))
        .await?;
        return Ok(());
    }

    let war_data = match response.json::<War>().await {
        Ok(war_data) => war_data,
        Err(err) => {
            log::warn!("could not read the war state for {}: {err}", guild.shard_name);
            ctx.say(format!(
                "Shard **{}** answered with something we couldn't read. Please report it if it \
                 keeps happening.",
                guild.shard_name
            ))
            .await?;
            return Ok(());
        }
    };

    let embed = serenity::CreateEmbed::new()
        .color((255, 0, 0))
        .field("Shard/Server", &guild.shard_name, false)
        .field("War Number", war_data.war_number.to_string(), false)
        .field("Winner", &war_data.winner, false)
        .field(
            "Conquest Start Time",
            format_timestamp(war_data.conquest_start_time),
            false,
        )
        .field(
            "Required Victory Towns",
            war_data.required_victory_towns.to_string(),
            false,
        )
        .footer(serenity::CreateEmbedFooter::new("Requested at"))
        .timestamp(serenity::Timestamp::now());

    ctx.send(poise::CreateReply::default().embed(embed)).await?;

    Ok(())
}
