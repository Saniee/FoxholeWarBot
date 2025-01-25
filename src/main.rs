use ftail::Ftail;
use serde::{Deserialize, Serialize};

use serenity::all::{ActivityData, CreateAutocompleteResponse};
use serenity::async_trait;
use serenity::builder::{CreateInteractionResponse, CreateInteractionResponseMessage, AutocompleteChoice};
use serenity::model::application::Interaction;
use serenity::model::gateway::Ready;
use serenity::model::id::GuildId;
use serenity::prelude::*;
use utils::api_definitions::foxhole::Maps;

mod utils;
mod commands;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Cfg {
    token: String,
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = &interaction {
            println!("Received command interaction!");

            let content = match command.data.name.as_str() {
                "get-map" => {
                    commands::get_map::run(&ctx, command).await.unwrap();
                    None
                },
                _ => Some("not implemented :(".to_string()),
            };

            if let Some(content) = content {
                let data = CreateInteractionResponseMessage::new().content(content);
                let builder = CreateInteractionResponse::Message(data);
                if let Err(why) = command.create_response(&ctx.http, builder).await {
                    println!("Cannot respond to slash command: {why}");
                }
            }
        }
        if let Interaction::Autocomplete(command) = &interaction {
            println!("Recieved Autocomplete Interaction");

            let client = reqwest::Client::new();

            let maps = client.get("https://war-service-live.foxholeservices.com/api/worldconquest/maps").send().await.unwrap().json::<Maps>().await.unwrap();

            let mut choices: Vec<AutocompleteChoice> = vec![];

            let filter = command.data.options[0].value.as_str().unwrap_or("").trim().to_lowercase();
            
            for str in maps {
                if str == "OriginHex" {
                    continue;
                }
                if str.trim().to_lowercase().contains(&filter) && choices.len() < 25 {
                    choices.push(AutocompleteChoice::new(str.replace("Hex", "").as_str(), str));
                }
            }
            let response = CreateInteractionResponse::Autocomplete(CreateAutocompleteResponse::new().set_choices(choices));

            let _ = command.create_response(&ctx.http, response).await;
        }
    }
    
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!\nIn {} guild/s!", ready.user.name, ready.guilds.len());

        ctx.set_activity(Some(ActivityData::watching("Foxhole Wars")));

        let guild_id = GuildId::new(
            dotenv::var("GUILD_ID")
                .expect("Expected GUILD_ID in environment")
                .parse()
                .expect("GUILD_ID must be an integer"),
        );

        let _ = guild_id
            .set_commands(&ctx.http, vec![
                commands::get_map::register(),
            ])
            .await;
    }
}

#[tokio::main]
async fn main() {
    Ftail::new().daily_file("logs", log::LevelFilter::Debug).init().unwrap();
    
    let token = dotenv::var("TOKEN").expect("No token string found in .env!");

    let mut client = Client::builder(token, GatewayIntents::empty())
        .event_handler(Handler)
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
