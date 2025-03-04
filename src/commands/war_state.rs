use serenity::all::{CommandInteraction, Context, CreateCommand, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage, EditInteractionResponse};

use crate::utils::api_definitions::foxhole::War;
use crate::utils::db::Database;

pub async fn run(ctx: &Context, interaction: &CommandInteraction, db: Database) -> Result<(), serenity::Error> {
    let guild_id = interaction.guild_id.unwrap().get();
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

    let request_client = reqwest::Client::new();
    let war_data = request_client.get(format!("{}/worldconquest/war", guild.shard)).send().await.unwrap().json::<War>().await.unwrap();

    let e = CreateEmbed::new().color((255, 0, 0))
        .field("Shard/Server", guild.shard_name, false)
        .field("War Number", war_data.war_number.to_string(), false)
        .field("Winner", war_data.winner, false)
        .field("Conquest Start Time", chrono::DateTime::from_timestamp_millis(war_data.conquest_start_time).unwrap().format("%Y %m %d %H:%M:%S").to_string(), false)
        .field("Required Victory Towns", war_data.required_victory_towns.to_string(), false).footer(CreateEmbedFooter::new("Requested at")).timestamp(chrono::Local::now());
    let msg = EditInteractionResponse::new().add_embed(e);

    interaction.edit_response(ctx, msg).await?;

    Ok(())
}

pub fn register() -> CreateCommand {
    CreateCommand::new("war-state").description("Gets the global state of the war. This command will default to not showing.")
}