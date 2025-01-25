use serenity::all::*;

use crate::utils::api_definitions::foxhole::{DynamicMapData, StaticMapData};
use crate::utils::image_processing::place_info;

pub async fn run(ctx: &Context, interaction: &CommandInteraction) -> Result<(), serenity::Error> {
    interaction.defer(ctx).await?;

    let map_name = interaction.data.options[0].value.as_str().unwrap();
    let draw_text = if interaction.data.options.len() > 1 {
        interaction.data.options[1].value.as_bool().unwrap()
    } else {
        false
    };

    let request_client = reqwest::Client::new();

    let dynamic_data = request_client.get(format!("https://war-service-live.foxholeservices.com/api/worldconquest/maps/{}/dynamic/public", map_name)).send().await.unwrap().json::<DynamicMapData>().await.unwrap();

    let static_data = request_client.get(format!("https://war-service-live.foxholeservices.com/api/worldconquest/maps/{}/static", map_name)).send().await.unwrap().json::<StaticMapData>().await.unwrap();

    let img = match place_info(dynamic_data, static_data, draw_text, &format!("./assets/Maps/Map{}.TGA", map_name)) {
        Some(i) => i,
        None => {
            interaction.edit_response(ctx, EditInteractionResponse::new().content("Couldn't find the map...")).await?;

            return Ok(());
        }
    };

    let _ = img.save("render.png");
    
    let e = CreateEmbed::new().color((0, 255, 0)).description(map_name).attachment("attachment://render.png").image("attachment://render.png").footer(CreateEmbedFooter::new("Requested At")).timestamp(chrono::Local::now());
    let msg = EditInteractionResponse::new().add_embed(e).new_attachment(CreateAttachment::path("render.png").await?);

    interaction.edit_response(ctx, msg).await?;
    
    Ok(())
}

pub fn register() -> CreateCommand {
    CreateCommand::new("get-map").description("test")
    .add_option(CreateCommandOption::new(CommandOptionType::String, "map-name", "Map name.").required(true).set_autocomplete(true))
    .add_option(CreateCommandOption::new(CommandOptionType::Boolean, "draw-text", "Display info text?").required(false))
}