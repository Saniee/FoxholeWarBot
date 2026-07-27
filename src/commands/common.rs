//! Helpers shared by the command modules: guild-settings lookup, visibility-aware
//! deferral, and the map-name autocomplete that three commands need.

use poise::serenity_prelude as serenity;

use crate::utils::cache::load_maps;
use crate::utils::db::GuildData;
use crate::utils::regions::display_name;
use crate::{Context, Error};

pub const NEEDS_SETUP: &str =
    "No shard is set for this server. Run `/set-guild-settings` to choose one.";

/// The one place the invite is written down in the bot. Keep it in the
/// `discord.gg` form the docs site uses, so the link users see in Discord and
/// the link on the Pages site are visibly the same.
pub const SUPPORT_INVITE: &str = "https://discord.gg/9wzppSgXdQ";

/// Discord rejects an autocomplete response with more than 25 choices.
const MAX_CHOICES: usize = 25;

/// The `map-name` value that means "the whole world map" rather than a region.
///
/// A sentinel in the same field, rather than a second `full:true` option, so
/// there is exactly one place a schedule says what it renders. It can't collide
/// with a region: every region identifier the API serves ends in `Hex`, and this
/// doesn't.
pub const FULL_MAP_TARGET: &str = "full-map";

/// Label for the sentinel, in the picker where a region name would be.
const FULL_MAP_LABEL: &str = "The whole world map (needs approval to schedule)";

/// The guild's Discord id as the `i64` the database stores.
///
/// Every command is `guild_only`, so poise rejects DM invocations before the
/// body runs — but `guild_id()` is still an `Option`, and unwrapping it is what
/// used to panic the whole handler (QA C-4).
pub fn guild_id(ctx: Context<'_>) -> Result<i64, Error> {
    let id = ctx
        .guild_id()
        .ok_or("this command can only be used in a server")?;

    Ok(id.get() as i64)
}

/// Loads the guild's settings, replying with the setup prompt if it has none.
/// `Ok(None)` means the user has already been told what to do.
pub async fn guild_settings(ctx: Context<'_>) -> Result<Option<GuildData>, Error> {
    let guild = ctx.data().db.get_guild(guild_id(ctx)?).await?;

    if guild.is_none() {
        ctx.send(poise::CreateReply::default().content(NEEDS_SETUP).ephemeral(true))
            .await?;
    }

    Ok(guild)
}

/// Defers according to the guild's output-visibility preference.
pub async fn defer_for(ctx: Context<'_>, guild: &GuildData) -> Result<(), Error> {
    if guild.show_command_output {
        ctx.defer().await?;
    } else {
        ctx.defer_ephemeral().await?;
    }

    Ok(())
}

/// Autocomplete over the shard's region list, labelled with real display names.
///
/// `OriginHex` is no longer filtered out: Origin is a live region, and the right
/// guard is "the API didn't list it", which this already is — the list comes
/// from `/worldconquest/maps` for the guild's own shard.
pub async fn autocomplete_map(
    ctx: Context<'_>,
    partial: &str,
) -> Vec<serenity::AutocompleteChoice> {
    let guild = match guild_id(ctx).ok() {
        Some(id) => ctx.data().db.get_guild(id).await.ok().flatten(),
        None => None,
    };

    let Some(guild) = guild else {
        // Discord rejects an empty choice value, so the placeholder has to be
        // non-empty even though picking it can only fail.
        return vec![serenity::AutocompleteChoice::new(
            "Run /set-guild-settings first for this to work!",
            "-",
        )];
    };

    let filter = partial.trim().to_lowercase();

    load_maps(guild.shard())
        .await
        .into_iter()
        .filter_map(|api_name| {
            let label = display_name(&api_name);

            // Match on both, so "mooring" and "the moors" both find The Moors.
            let matches = label.to_lowercase().contains(&filter)
                || api_name.to_lowercase().contains(&filter);

            matches.then(|| serenity::AutocompleteChoice::new(label, api_name))
        })
        .take(MAX_CHOICES)
        .collect()
}

/// The same list, plus the whole-world-map option.
///
/// Only `/schedule-report` uses this. `/get-map` and `/war-report` take a single
/// region by definition, and offering them a target they can't render would be a
/// picker entry whose only outcome is an error.
pub async fn autocomplete_schedule_target(
    ctx: Context<'_>,
    partial: &str,
) -> Vec<serenity::AutocompleteChoice> {
    let mut choices = autocomplete_map(ctx, partial).await;

    let filter = partial.trim().to_lowercase();
    let matches = filter.is_empty()
        || FULL_MAP_LABEL.to_lowercase().contains(&filter)
        || FULL_MAP_TARGET.contains(&filter);

    if matches {
        // First, so it's visible without scrolling — and truncated back to
        // Discord's limit, since the region list may already be full.
        choices.insert(
            0,
            serenity::AutocompleteChoice::new(FULL_MAP_LABEL, FULL_MAP_TARGET),
        );
        choices.truncate(MAX_CHOICES);
    }

    choices
}
