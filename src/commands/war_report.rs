use poise::serenity_prelude as serenity;
use reqwest::StatusCode;

use crate::commands::common::{
    autocomplete_map, defer_for, guild_settings, placeholder_submitted, unreachable_message,
};
use crate::utils::api_definitions::foxhole::WarReport;
use crate::utils::cache::{load_war_report, save_war_report};
use crate::utils::http;
use crate::utils::regions::display_name;
use crate::{Context, Error};

/// Responds with the war report for a single region.
#[poise::command(slash_command, guild_only)]
pub async fn war_report(
    ctx: Context<'_>,
    #[description = "Name of the region you want a report for."]
    #[autocomplete = "autocomplete_map"]
    map_name: String,
) -> Result<(), Error> {
    let Some(guild) = guild_settings(ctx).await? else {
        return Ok(());
    };

    defer_for(ctx, &guild).await?;

    if placeholder_submitted(ctx, &map_name).await? {
        return Ok(());
    }

    let cached = load_war_report(&map_name, &guild.shard_name).await;

    // Not `?`. A shard that has gone offline since it was set up fails here at
    // the transport layer, and propagating that reaches the user as "Something
    // went wrong running that command" — true, unactionable, and identical to
    // the message for a genuine bug.
    let response = match http::client()
        .get(format!(
            "{}/worldconquest/warReport/{map_name}",
            guild.shard
        ))
        .header(
            "If-None-Match",
            format!("\"{}\"", cached.as_ref().map_or(0, |r| r.version)),
        )
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            log::warn!("could not reach shard {} for a war report: {err}", guild.shard_name);
            ctx.say(unreachable_message(&guild.shard_name)).await?;
            return Ok(());
        }
    };

    let report = match response.status() {
        StatusCode::NOT_MODIFIED => match cached {
            Some(report) => report,
            // Our cache went away between the read and the request; refetch
            // unconditionally rather than unwrapping a None.
            None => match refetch(&guild.shard, &map_name).await {
                Ok(report) => report,
                Err(err) => {
                    log::warn!("could not refetch the war report for {map_name}: {err}");
                    ctx.say(unreachable_message(&guild.shard_name)).await?;
                    return Ok(());
                }
            },
        },
        StatusCode::OK => {
            let report = response.json::<WarReport>().await?;
            save_war_report(&report, &map_name, &guild.shard_name).await;
            report
        }
        status => match cached {
            Some(report) => {
                log::warn!("the Foxhole API returned {status}, serving the cached war report");
                report
            }
            None => {
                ctx.say(format!(
                    "The Foxhole API returned `{status}` for that region. Try again shortly."
                ))
                .await?;
                return Ok(());
            }
        },
    };

    let embed = serenity::CreateEmbed::new()
        .color((0, 0, 0))
        .title(display_name(&map_name))
        .field("Total Enlistments", report.total_enlistments.to_string(), false)
        .field(
            "Colonial Casualties",
            report.colonial_casualties.to_string(),
            false,
        )
        .field(
            "Warden Casualties",
            report.warden_casualties.to_string(),
            false,
        )
        .field("Day Of War", report.day_of_war.to_string(), false)
        .footer(serenity::CreateEmbedFooter::new("Requested at"))
        .timestamp(serenity::Timestamp::now());

    ctx.send(poise::CreateReply::default().embed(embed)).await?;

    Ok(())
}

async fn refetch(api_url: &str, map_name: &str) -> Result<WarReport, Error> {
    let report = http::client()
        .get(format!("{api_url}/worldconquest/warReport/{map_name}"))
        .send()
        .await?
        .error_for_status()?
        .json::<WarReport>()
        .await?;

    Ok(report)
}
