use poise::serenity_prelude as serenity;

use crate::commands::common::{defer_for, guild_settings};
use crate::utils::api_definitions::foxhole::War;
use crate::utils::format_timestamp;
use crate::{Context, Error};

/// Gets the global state of the war on this server's shard.
#[poise::command(slash_command, guild_only)]
pub async fn war_state(ctx: Context<'_>) -> Result<(), Error> {
    let Some(guild) = guild_settings(ctx).await? else {
        return Ok(());
    };

    defer_for(ctx, &guild).await?;

    let war_data = reqwest::Client::new()
        .get(format!("{}/worldconquest/war", guild.shard))
        .send()
        .await?
        .error_for_status()?
        .json::<War>()
        .await?;

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
