use poise::serenity_prelude as serenity;
use reqwest::StatusCode;

use crate::commands::common::{autocomplete_map, defer_for, guild_settings};
use crate::utils::api_definitions::foxhole::WarReport;
use crate::utils::cache::{load_war_report, save_war_report};
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

    let cached = load_war_report(&map_name, &guild.shard_name).await;

    let response = reqwest::Client::new()
        .get(format!(
            "{}/worldconquest/warReport/{map_name}",
            guild.shard
        ))
        .header(
            "If-None-Match",
            format!("\"{}\"", cached.as_ref().map_or(0, |r| r.version)),
        )
        .send()
        .await?;

    let report = match response.status() {
        StatusCode::NOT_MODIFIED => match cached {
            Some(report) => report,
            // Our cache went away between the read and the request; refetch
            // unconditionally rather than unwrapping a None.
            None => refetch(&guild.shard, &map_name).await?,
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
    let report = reqwest::Client::new()
        .get(format!("{api_url}/worldconquest/warReport/{map_name}"))
        .send()
        .await?
        .error_for_status()?
        .json::<WarReport>()
        .await?;

    Ok(report)
}
