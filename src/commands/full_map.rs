use poise::serenity_prelude as serenity;

use crate::commands::common::{defer_for, guild_settings};
use crate::utils::format_timestamp;
use crate::utils::map_render::{render_full_map, MapError};
use crate::utils::regions::REGIONS;
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

    // The tint is honored here only: a single `/get-map` hex is looked at
    // closely enough that a colour wash costs more than it tells you. The
    // frontline is honored on both, because the contested hex is precisely the
    // one a per-hex answer serves worst.
    let config = RenderConfig {
        faction_tint: guild.full_map_faction_tint,
        frontline: guild.frontline,
        ..RenderConfig::default()
    };

    // Posted before the work starts. A full map is 53 fetches and a 63.6 MP
    // composite, which is long enough that a bare spinner reads as a command
    // that failed silently — see `specs/full-map-renderer.md` → Saying what it
    // is doing for why this is one message rather than a moving one.
    let status = ctx
        .say(rendering_message(
            &guild.shard_name,
            guild.frontline,
            guild.full_map_faction_tint,
        ))
        .await?;

    let rendered = match render_full_map(&guild.shard, &guild.shard_name, false, config).await {
        Ok(rendered) => rendered,
        // Every failure path replies. A deferred interaction that never gets a
        // final response leaves the user staring at a spinner forever (QA C-12),
        // and now also at a status line that never resolves — so these edit that
        // message rather than leaving it above a second one.
        //
        // Every region failed, which on a full map means the shard itself, not
        // one bad hex — so name it. A guild pointed at a shard that has gone
        // offline gets this every time, and "the API" gives them nothing to act
        // on while "shard Charlie" points straight at `/set-guild-settings`.
        Err(MapError::ApiUnavailable) => {
            return finish(
                ctx,
                &status,
                poise::CreateReply::default().content(format!(
                    "Couldn't get any region data from shard **{}** — it looks to be down or \
                     between wars. Try again shortly, or switch shards with \
                     `/set-guild-settings`.",
                    guild.shard_name
                )),
            )
            .await;
        }
        Err(err @ MapError::TooLarge(_)) => {
            log::warn!("full map for {}: {err}", guild.shard_name);

            return finish(
                ctx,
                &status,
                poise::CreateReply::default()
                    .content("The full map came out too large to upload. Please report this."),
            )
            .await;
        }
        Err(err) => {
            log::warn!("could not render the full map for {}: {err}", guild.shard_name);

            return finish(
                ctx,
                &status,
                poise::CreateReply::default()
                    .content("Couldn't render the full map. Try again shortly."),
            )
            .await;
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

    finish(
        ctx,
        &status,
        poise::CreateReply::default()
            .embed(embed)
            .attachment(serenity::CreateAttachment::bytes(rendered.png, file_name)),
    )
    .await
}

/// Replaces the status message with whatever the render produced.
///
/// Blanking the content is not optional and is the reason this exists rather
/// than being written out three times: an edit leaves fields it does not mention
/// alone, so without it the "Rendering…" text would sit above the finished map
/// forever.
async fn finish(
    ctx: Context<'_>,
    status: &poise::ReplyHandle<'_>,
    reply: poise::CreateReply,
) -> Result<(), Error> {
    status.edit(ctx, reply.content("")).await?;

    Ok(())
}

/// What the bot says while it works.
///
/// **One message, posted once, and never updated in place.** A moving progress
/// bar was the obvious design and is the wrong one here: editing an interaction
/// response counts against Discord's rate limits, and a render reports something
/// worth showing about a hundred times — so a live bar means either throttling it
/// down to a handful of edits that barely move, or spending the guild's limit on
/// decoration. This says the same thing in one call.
///
/// It is detailed rather than terse for the same reason it exists: the wait is
/// unexplained otherwise, and "53 regions" is what makes it obviously
/// proportionate rather than obviously stuck. The two settings are named because
/// they are the ones that change what comes back, and a user who forgot they
/// turned them on has no other way to tell from the finished image whether they
/// took.
fn rendering_message(shard_name: &str, frontline: bool, faction_tint: bool) -> String {
    let mut message = format!(
        "**Rendering the world map for {shard_name}.**\nStitching all {} regions onto one canvas \
         — this takes a moment.",
        REGIONS.len()
    );

    let extras = match (frontline, faction_tint) {
        (true, true) => Some("frontline and territory tint"),
        (true, false) => Some("frontline"),
        (false, true) => Some("territory tint"),
        (false, false) => None,
    };

    if let Some(extras) = extras {
        message.push_str(&format!("\nWith the {extras} on."));
    }

    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_message_names_the_shard_and_the_scale_of_the_job() {
        let message = rendering_message("Able", false, false);

        assert!(message.contains("Able"), "{message}");
        assert!(message.contains("53 regions"), "{message}");
        assert!(
            !message.contains("With the"),
            "nothing to name when both are off: {message}"
        );
    }

    #[test]
    fn the_message_names_whichever_overlays_are_on() {
        assert!(rendering_message("Able", true, false).ends_with("With the frontline on."));
        assert!(rendering_message("Able", false, true).ends_with("With the territory tint on."));
        assert!(rendering_message("Able", true, true)
            .ends_with("With the frontline and territory tint on."));
    }
}
