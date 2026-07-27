//! Helpers shared by the command modules: guild-settings lookup, visibility-aware
//! deferral, and the map-name autocomplete that three commands need.

use poise::serenity_prelude as serenity;

use crate::utils::cache::load_maps;
use crate::utils::db::GuildData;
use crate::utils::regions::display_name;
use crate::{Context, Error};

pub const NEEDS_SETUP: &str =
    "No shard is set for this server. Run `/set-guild-settings` to choose one.";

pub const SUPPORT_INVITE: &str = "https://discord.com/invite/9wzppSgXdQ";

/// Discord rejects an autocomplete response with more than 25 choices.
const MAX_CHOICES: usize = 25;

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
        return vec![serenity::AutocompleteChoice::new(
            "Run /set-guild-settings first for this to work!",
            "",
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
