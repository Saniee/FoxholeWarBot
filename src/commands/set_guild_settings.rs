use reqwest::StatusCode;
use serenity::all::{AutocompleteChoice, CommandInteraction, CommandOptionType, Context, CreateAutocompleteResponse, CreateCommand, CreateCommandOption, CreateInteractionResponse, EditInteractionResponse, Permissions};

use crate::utils::db::{Database, Shard};

pub const NAME: &str = "set-guild-settings";

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

    let guild_id = interaction.guild_id.unwrap().get().try_into().unwrap();

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

pub async fn autocomplete(ctx: &Context, interaction: &CommandInteraction) -> Result<(), serenity::Error> {
    let choices: Vec<AutocompleteChoice> = vec![AutocompleteChoice::new("Able", "Able"), AutocompleteChoice::new("Baker", "Baker"), AutocompleteChoice::new("Charlie", "Charlie")];

    let response = CreateInteractionResponse::Autocomplete(CreateAutocompleteResponse::new().set_choices(choices));

    interaction.create_response(&ctx.http, response).await?;
    
    Ok(())
}

pub fn register() -> CreateCommand {
    CreateCommand::new(NAME).default_member_permissions(Permissions::ADMINISTRATOR).description("Sets the server the bot will get data from and whether or not messages will show to others.").add_option(CreateCommandOption::new(CommandOptionType::String, "shard", "Choose which server to get data from.").set_autocomplete(true).required(true)).add_option(CreateCommandOption::new(CommandOptionType::Boolean, "show-messages", "True shows commands to everyone. False to only the one who called the command.").required(true))
}