use reqwest::StatusCode;
use serenity::all::{AutocompleteChoice, CommandInteraction, CommandOptionType, Context, CreateAutocompleteResponse, CreateCommand, CreateCommandOption, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage, EditInteractionResponse};

use crate::utils::api_definitions::foxhole::WarReport;
use crate::utils::cache::{load_war_report, load_maps, save_war_report};
use crate::utils::db::{Database, Shard};

pub async fn run(ctx: &Context, interaction: &CommandInteraction, db: Database) -> Result<(), serenity::Error> {
    let guild_id = interaction.guild_id.unwrap().get();
    let data = db.get_guild(guild_id).await;

    let guild = match data {
        None => {
            interaction.create_response(ctx, CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().ephemeral(true).content("No Shard set for the guild. Run the `/set-shard` command to do so."))).await?;
            return Ok(());
        }
        Some(data) => data
    };
    
    if guild.show_command_output == 1 {
        interaction.defer(ctx).await?;
    } else if guild.show_command_output == 0 {
        interaction.defer_ephemeral(ctx).await?;
    }

    let map_name = interaction.data.options[0].value.as_str().unwrap();

    let api_url: String = guild.shard;

    let request_client = reqwest::Client::new();

    let cache_data = match load_war_report(map_name, &guild.shard_name).await {
        Some(data) => Some(data),
        None => {
            println!("No cached data found... Creating...");
            None
        },
    };

    #[allow(clippy::needless_late_init)]
    let war_report_res;

    if cache_data.is_none() {
        war_report_res = request_client.get(format!("{}/worldconquest/warReport/{}", api_url, map_name)).header("If-None-Match", "\"0\"").send().await.unwrap();
    } else {
        war_report_res = request_client.get(format!("{}/worldconquest/warReport/{}", api_url, map_name)).header("If-None-Match", format!("\"{}\"", cache_data.clone().unwrap().version)).send().await.unwrap();
    }

    if war_report_res.status() == StatusCode::INTERNAL_SERVER_ERROR {
        return Ok(());
    }

    if war_report_res.status() != StatusCode::NOT_MODIFIED {
        let war_report_data = war_report_res.json::<WarReport>().await.unwrap();

        let e = CreateEmbed::new().color((0, 0, 0))
            .field("Total Enlistments", war_report_data.clone().total_enlistments.to_string(), false)
            .field("Colonial Casualties", war_report_data.clone().colonial_casualties.to_string(), false)
            .field("Warden Casualties", war_report_data.clone().warden_casualties.to_string(), false)
            .field("Day Of War", war_report_data.clone().day_of_war.to_string(), false)
            .footer(CreateEmbedFooter::new("Requested at")).timestamp(chrono::Local::now());
        let msg = EditInteractionResponse::new().add_embed(e);

        save_war_report(war_report_data, map_name, &guild.shard_name).await;
        interaction.edit_response(ctx, msg).await?;

        Ok(())
    } else {   
        let e = CreateEmbed::new().color((0, 0, 0))
            .field("Total Enlistments", cache_data.clone().unwrap().total_enlistments.to_string(), false)
            .field("Colonial Casualties", cache_data.clone().unwrap().colonial_casualties.to_string(), false)
            .field("Warden Casualties", cache_data.clone().unwrap().warden_casualties.to_string(), false)
            .field("Day Of War", cache_data.clone().unwrap().day_of_war.to_string(), false)
            .footer(CreateEmbedFooter::new("Requested at")).timestamp(chrono::Local::now());
        let msg = EditInteractionResponse::new().add_embed(e);
        
        interaction.edit_response(ctx, msg).await?; 

        Ok(())
    }
}

pub async fn autocomplete(ctx: &Context, interaction: &CommandInteraction, db: Database) -> Result<(), serenity::Error> {
    let guild_id = interaction.guild_id.unwrap().get();
    let data = db.get_guild(guild_id).await;

    let mut choices: Vec<AutocompleteChoice> = vec![];

    let guild = match data {
        None => {
            choices.push(AutocompleteChoice::new("Please Run the /set-shard command for this to work!", ""));
            interaction.create_response(&ctx.http, CreateInteractionResponse::Autocomplete(CreateAutocompleteResponse::new().set_choices(choices))).await?;
            return Ok(());
        },
        Some(guild) => guild
    };
    
    let maps = load_maps(Shard::from_str(&guild.shard_name)).await;

    let filter = interaction.data.options[0].value.as_str().unwrap_or("").trim().to_lowercase();
    
    for str in maps {
        if str == "OriginHex" {
            continue;
        }
        if str.trim().to_lowercase().contains(&filter) && choices.len() < 25 {
            choices.push(AutocompleteChoice::new(str.replace("Hex", "").as_str(), str));
        }
    }
    let response = CreateInteractionResponse::Autocomplete(CreateAutocompleteResponse::new().set_choices(choices));

    interaction.create_response(&ctx.http, response).await?;
    Ok(())
}

pub fn register() -> CreateCommand {
    CreateCommand::new("war-report").description("Responds with information about a Hex/Map Chunk.")
    .add_option(CreateCommandOption::new(CommandOptionType::String, "map-name", "Name of the Hex you want displayed.").required(true).set_autocomplete(true))
}