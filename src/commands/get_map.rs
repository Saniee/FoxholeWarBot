use reqwest::StatusCode;
use serenity::all::{AutocompleteChoice, CommandInteraction, CommandOptionType, Context, CreateAttachment, CreateAutocompleteResponse, CreateCommand, CreateCommandOption, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage, EditInteractionResponse};

use crate::utils::api_definitions::foxhole::{DynamicMapData, StaticMapData};
use crate::utils::cache::{load_map_cache, load_maps, save_map_cache};
use crate::utils::db::{Database, Shard};
use crate::utils::request_processing::place_image_info;

pub const NAME: &str = "get-map";

pub async fn run(ctx: &Context, interaction: &CommandInteraction, db: Database) -> Result<(), serenity::Error> {
    let guild_id = interaction.guild_id.unwrap().get().try_into().unwrap();
    let data = db.get_guild(guild_id).await;

    let guild = match data {
        None => {
            interaction.create_response(ctx, CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().ephemeral(true).content("No Shard set for the guild. Run the `/set-guild-settings` command to do so."))).await?;
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

    let draw_text = if interaction.data.options.len() > 1 {
        interaction.data.options[1].value.as_bool().unwrap()
    } else {
        false
    };

    let request_client = reqwest::Client::new();

    let cache_data = match load_map_cache(map_name, &guild.shard_name).await {
        Some(data) => Some(data),
        None => {
            println!("No cached data found... Creating...");
            None
        },
    }; 

    let dynamic_response;
    let static_response;
    let last_updated;

    if cache_data.is_none() {
        dynamic_response = request_client.get(format!("{api_url}/worldconquest/maps/{map_name}/dynamic/public")).header("If-None-Match", "\"0\"").send().await.unwrap();
        
        static_response = request_client.get(format!("{api_url}/worldconquest/maps/{map_name}/static")).header("If-None-Match", "\"0\"").send().await.unwrap();

        //println!("{} | {}", dynamic_response.status(), static_response.status());
    } else {
        dynamic_response = request_client.get(format!("{api_url}/worldconquest/maps/{map_name}/dynamic/public")).header("If-None-Match", format!("\"{}\"", cache_data.clone().unwrap().0.version)).send().await.unwrap();
        
        static_response = request_client.get(format!("{api_url}/worldconquest/maps/{map_name}/static")).header("If-None-Match", format!("\"{}\"", cache_data.clone().unwrap().1.version)).send().await.unwrap();
    }

    if dynamic_response.status() == StatusCode::INTERNAL_SERVER_ERROR && static_response.status() == StatusCode::INTERNAL_SERVER_ERROR {
        return Ok(());
    }

    if dynamic_response.status() != StatusCode::NOT_MODIFIED && static_response.status() != StatusCode::NOT_MODIFIED {
        let dynamic_data = dynamic_response.json::<DynamicMapData>().await.unwrap();
        last_updated = dynamic_data.last_updated;

        let static_data = static_response.json::<StaticMapData>().await.unwrap();

        let img = match place_image_info(&dynamic_data, &static_data, draw_text, &format!("./assets/Maps/Map{map_name}.TGA")) {
            Some(i) => i,
            None => {
                interaction.edit_response(ctx, EditInteractionResponse::new().content("Couldn't find the map... ||Or an error occured...||")).await?;
    
                return Ok(());
            }
        };
    
        let _ = img.save("render.png");
        save_map_cache(dynamic_data, static_data, map_name, &guild.shard_name).await;
    } else {
        last_updated = cache_data.clone().unwrap().0.last_updated;
        let img = match place_image_info(&cache_data.clone().unwrap().0, &cache_data.clone().unwrap().1, draw_text, &format!("./assets/Maps/Map{map_name}.TGA")) {
            Some(i) => i,
            None => {
                interaction.edit_response(ctx, EditInteractionResponse::new().content("Couldn't find the map...")).await?;
    
                return Ok(());
            }
        };
    
        let _ = img.save("render.png");
    }
    
    let e = CreateEmbed::new().color((0, 255, 0)).description(format!("Last API Update: {}", chrono::DateTime::from_timestamp_millis(last_updated).unwrap().format("%Y %m %d %H:%M:%S"))).attachment("attachment://render.png").image("attachment://render.png").footer(CreateEmbedFooter::new("Requested at")).timestamp(chrono::Local::now());
    let msg = EditInteractionResponse::new().add_embed(e).new_attachment(CreateAttachment::path("render.png").await?);

    interaction.edit_response(ctx, msg).await?;
    
    Ok(())
}

pub async fn autocomplete(ctx: &Context, interaction: &CommandInteraction, db: Database) -> Result<(), serenity::Error> {
    let guild_id = interaction.guild_id.unwrap().get().try_into().unwrap();
    let data = db.get_guild(guild_id).await;

    let mut choices: Vec<AutocompleteChoice> = vec![];

    let guild = match data {
        None => {
            choices.push(AutocompleteChoice::new("Please Run the /set-guild-settings command for this to work!", ""));
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
    CreateCommand::new(NAME).description("Responds with an image of that Hex/Map chunk. With Colored icons and labels.")
    .add_option(CreateCommandOption::new(CommandOptionType::String, "map-name", "Name of the Hex you want displayed.").required(true).set_autocomplete(true))
    .add_option(CreateCommandOption::new(CommandOptionType::Boolean, "draw-text", "Renders Text for things on the map. Turned off by default, since it makes the render messy.").required(false))
}