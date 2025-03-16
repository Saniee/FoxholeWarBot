use reqwest::StatusCode;
use serenity::all::{CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption, EditInteractionResponse, Permissions};

use crate::utils::db::{Database, Shard};

pub async fn run(ctx: &Context, interaction: &CommandInteraction, db: Database) -> Result<(), serenity::Error> {
    interaction.defer_ephemeral(ctx).await?;

    let shard_name = interaction.data.options[0].value.as_str().unwrap();
    let show = if interaction.data.options.len() > 1 {
        if interaction.data.options[1].value.as_bool().unwrap() {
            1
        } else {
            0
        }
    } else {
        0
    };

    let guild_id = interaction.guild_id.unwrap().get();

    let data = db.get_guild(guild_id).await;

    let request_client = reqwest::Client::new();

    match data {
        None => {
            let resp = request_client.get(format!("{}/worldconquest/war", Shard::from_str(shard_name).api_url())).send().await.unwrap();

            match resp.status() {
                StatusCode::OK => {
                    db.create_guild(guild_id, Some(shard_name)).await;
                    interaction.edit_response(ctx, EditInteractionResponse::new().content("Created!")).await?;
                }
                StatusCode::SERVICE_UNAVAILABLE => {
                    println!("{:?}", resp);
                    interaction.edit_response(ctx, EditInteractionResponse::new().content("Cannot set as the server selected is Unavailable!")).await?;
                }
                _ => {
                    interaction.edit_response(ctx, EditInteractionResponse::new().content("An Error Occured!")).await?;
                    println!("{}", resp.status());
                }
            }

            Ok(())
        },
        Some(data) => {
            let resp = request_client.get(format!("{}/worldconquest/war", Shard::from_str(shard_name).api_url())).send().await.unwrap();

            match resp.status() {
                StatusCode::OK => {
                    db.update_guild(data.id, shard_name, show).await;
                    interaction.edit_response(ctx, EditInteractionResponse::new().content("Updated!")).await?;
                }
                StatusCode::SERVICE_UNAVAILABLE => {
                    println!("{:?}", resp);
                    interaction.edit_response(ctx, EditInteractionResponse::new().content("Cannot set as the server selected is Unavailable!")).await?;
                }
                _ => {
                    interaction.edit_response(ctx, EditInteractionResponse::new().content("An Error Occured!")).await?;
                    println!("{}", resp.status());
                }
            }

            Ok(())
        }
    }
}

pub fn register() -> CreateCommand {
    CreateCommand::new("set-guild-settings").default_member_permissions(Permissions::ADMINISTRATOR).description("Sets the server the bot will get data from and whether or not messages will show to others.")
    .add_option(CreateCommandOption::new(CommandOptionType::String, "shard", "Choose which server to get data from.").required(true).add_string_choice("Able", "Able").add_string_choice("Baker", "Baker").add_string_choice("Charlie", "Charlie"))
    .add_option(CreateCommandOption::new(CommandOptionType::Boolean, "show-messages", "True shows commands to everyone. False to only the one who called the command.").required(true))
}