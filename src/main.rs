use ftail::Ftail;

use serenity::all::ActivityData;
use serenity::async_trait;
use serenity::builder::{CreateInteractionResponse, CreateInteractionResponseMessage};
use serenity::model::application::Interaction;
use serenity::model::gateway::Ready;
use serenity::model::id::GuildId;
use serenity::prelude::*;
use utils::cache::save_maps_cache;

mod utils;
mod commands;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = &interaction {
            // println!("Received command interaction!");

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
            // println!("Recieved Autocomplete Interaction");

            match command.data.name.as_str() {
                "get-map" => {
                    commands::get_map::autocomplete(&ctx, command).await.unwrap();
                },
                _ => return
            }
        }
    }
    
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!\nIn {} guild/s!", ready.user.name, ready.guilds.len());

        ctx.set_activity(Some(ActivityData::watching("Foxhole Wars")));

        save_maps_cache().await;

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

        // Command::create_global_command(&ctx.http, commands::get_map::register()).await;
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
