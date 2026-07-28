use poise::serenity_prelude as serenity;

use crate::commands::common::{
    autocomplete_map, defer_for, guild_settings, placeholder_submitted,
};
use crate::utils::format_timestamp;
use crate::utils::map_render::{render_region, MapError};
use crate::utils::regions::display_name;
use crate::utils::request_processing::RenderConfig;
use crate::{Context, Error};

/// Renders a region's map with colored icons and optional labels.
#[poise::command(slash_command, guild_only)]
pub async fn get_map(
    ctx: Context<'_>,
    #[description = "Name of the region you want displayed."]
    #[autocomplete = "autocomplete_map"]
    map_name: String,
    #[description = "Draw map labels. Off by default — it makes the render busy."]
    draw_text: Option<bool>,
) -> Result<(), Error> {
    let Some(guild) = guild_settings(ctx).await? else {
        return Ok(());
    };

    defer_for(ctx, &guild).await?;

    if placeholder_submitted(ctx, &map_name).await? {
        return Ok(());
    }

    let rendered = match render_region(
        &guild.shard,
        &guild.shard_name,
        &map_name,
        draw_text.unwrap_or(false),
        RenderConfig::default(),
    )
    .await
    {
        Ok(rendered) => rendered,
        // Every failure path replies. A deferred interaction that never gets a
        // final response leaves the user staring at a spinner forever (QA C-12).
        Err(MapError::ApiUnavailable) => {
            ctx.say(format!(
                "Shard **{}** isn't responding right now. Try again shortly, or switch shards \
                 with `/set-guild-settings`.",
                guild.shard_name
            ))
            .await?;
            return Ok(());
        }
        Err(err @ MapError::Render(_)) => {
            log::warn!("could not render {map_name}: {err}");
            ctx.say(format!(
                "Couldn't render that region. If `{}` should exist, please report it.",
                display_name(&map_name)
            ))
            .await?;
            return Ok(());
        }
        Err(err) => {
            log::warn!("could not fetch {map_name}: {err}");
            ctx.say("Couldn't fetch that region's data. Try again shortly.")
                .await?;
            return Ok(());
        }
    };

    let file_name = format!("{map_name}.png");
    let attachment_url = format!("attachment://{file_name}");

    let embed = serenity::CreateEmbed::new()
        .color((0, 255, 0))
        .title(display_name(&map_name))
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
