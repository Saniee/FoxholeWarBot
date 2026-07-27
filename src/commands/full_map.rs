use poise::serenity_prelude as serenity;

use crate::commands::common::{defer_for, guild_settings};
use crate::utils::format_timestamp;
use crate::utils::map_render::{render_full_map, MapError};
use crate::utils::request_processing::RenderConfig;
use crate::{Context, Error};

/// Renders the whole world map: all 53 regions on one hex grid.
///
/// Ungated on purpose. A one-off render costs the host one burst of work that the
/// user explicitly asked for; it is *scheduling* one that recurs forever, and
/// that is what needs approval (`specs/premium-full-map.md`).
#[poise::command(slash_command, guild_only)]
pub async fn full_map(ctx: Context<'_>) -> Result<(), Error> {
    let Some(guild) = guild_settings(ctx).await? else {
        return Ok(());
    };

    defer_for(ctx, &guild).await?;

    // The tint is the one render setting a guild owns, and it is honored here
    // only: a single `/get-map` hex is looked at closely enough that a colour
    // wash costs more than it tells you.
    let config = RenderConfig {
        faction_tint: guild.full_map_faction_tint,
        ..RenderConfig::default()
    };

    // No labels: at the composite's downscale, in-region text is a smudge. A
    // per-hex region name drawn *after* the downscale is the v2 answer.
    let rendered = match render_full_map(&guild.shard, &guild.shard_name, false, config).await {
        Ok(rendered) => rendered,
        // Every failure path replies. A deferred interaction that never gets a
        // final response leaves the user staring at a spinner forever (QA C-12).
        Err(MapError::ApiUnavailable) => {
            ctx.say("The Foxhole API is not responding right now. Try again shortly.")
                .await?;
            return Ok(());
        }
        Err(err @ MapError::TooLarge(_)) => {
            log::warn!("full map for {}: {err}", guild.shard_name);
            ctx.say("The full map came out too large to upload. Please report this.")
                .await?;
            return Ok(());
        }
        Err(err) => {
            log::warn!("could not render the full map for {}: {err}", guild.shard_name);
            ctx.say("Couldn't render the full map. Try again shortly.")
                .await?;
            return Ok(());
        }
    };

    let file_name = format!("full-map-{}.png", guild.shard_name.to_lowercase());
    let attachment_url = format!("attachment://{file_name}");

    let embed = serenity::CreateEmbed::new()
        .color((0, 255, 0))
        .title(format!("World Conquest — {}", guild.shard_name))
        .description(format!(
            "Last API Update: {}",
            format_timestamp(rendered.last_updated)
        ))
        .image(&attachment_url)
        .footer(serenity::CreateEmbedFooter::new("Requested at"))
        .timestamp(serenity::Timestamp::now());

    ctx.send(
        poise::CreateReply::default()
            .embed(embed)
            .attachment(serenity::CreateAttachment::bytes(rendered.png, file_name)),
    )
    .await?;

    Ok(())
}
