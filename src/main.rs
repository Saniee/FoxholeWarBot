use args::Args;
use clap::Parser;
use ftail::Ftail;

use serenity::all::{ActivityData, Command, Guild, UnavailableGuild};
use serenity::async_trait;
use serenity::builder::{CreateInteractionResponse, CreateInteractionResponseMessage};
use serenity::model::application::Interaction;
use serenity::model::gateway::Ready;
use serenity::model::id::GuildId;
use serenity::prelude::*;
use sqlx::Executor;
use utils::cache::save_maps_cache;
use utils::db::Database;

mod utils;
mod commands;
mod args;

struct Handler {
    db: Database
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = &interaction {
            // println!("Received command interaction!");

            let content = match command.data.name.as_str() {
                "get-map" => {
                    commands::get_map::run(&ctx, command, self.db.clone()).await.unwrap();
                    None
                },
                "war-report" => {
                    commands::war_report::run(&ctx, command, self.db.clone()).await.unwrap();
                    None
                },
                "war-state" => {
                    commands::war_state::run(&ctx, command, self.db.clone()).await.unwrap();
                    None
                },
                "set-guild-settings" => {
                    commands::set_guild_settings::run(&ctx, command, self.db.clone()).await.unwrap();
                    None
                }
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
                    commands::get_map::autocomplete(&ctx, command, self.db.clone()).await.unwrap();
                },
                "war-report" => {
                    commands::war_report::autocomplete(&ctx, command, self.db.clone()).await.unwrap();
                },
                "set-guild-settings" => {
                    commands::set_guild_settings::autocomplete(&ctx, command).await.unwrap();
                }
                _ => return
            }
        }
    }
    
    async fn guild_create(&self, _ctx: Context, guild: Guild, is_new: Option<bool>) {
        let new = match is_new {
            Some(new) => new,
            None => return
        };

        if new {
            println!("Joined {}. Now in {} Guilds!", guild.name, _ctx.cache.guilds().len());
            self.db.create_guild(guild.id.get(), None).await;
        }
    }

    async fn guild_delete(&self, _ctx: Context, incomplete: UnavailableGuild, full: Option<Guild>) {
        
        if full.is_some() {
            if !incomplete.unavailable {
                println!("Left {}. Now in {} Guilds!", full.clone().unwrap().name, _ctx.cache.guilds().len());
                self.db.delete_guild(full.clone().unwrap().id.get()).await;
            }
        } else if !incomplete.unavailable {
            println!("Left {} (Incomplete Data). Now in {} Guilds!", incomplete.id, _ctx.cache.guilds().len());
            self.db.delete_guild(incomplete.id.get()).await;
        }
    }
    
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!\nIn {} guild/s!", ready.user.name, ready.guilds.len());

        ctx.set_activity(Some(ActivityData::watching("Foxhole Wars")));
        ctx.idle();

        save_maps_cache().await;

        let _ = GuildId::new(
            dotenv::var("GUILD_ID")
                .expect("Expected GUILD_ID in environment")
                .parse()
                .expect("GUILD_ID must be an integer"),
        );

        /* let _ = guild_id
            .set_commands(&ctx.http, vec![
                commands::get_map::register(),
                commands::war_report::register(),
                commands::war_state::register(),
                commands::set_guild_settings::register(),
            ])
            .await; */

        let _ = Command::set_global_commands(&ctx.http, vec![
            commands::get_map::register(),
            commands::war_report::register(),
            commands::war_state::register(),
            commands::set_guild_settings::register(),
        ]).await;
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Clear All Commands on App
    if args.clear_commands {
        let token = dotenv::var("TOKEN").expect("No token string found in .env!");
        let app_id = dotenv::var("APP_ID").expect("No app id found in .env!").parse().unwrap();

        let client = Client::builder(token, GatewayIntents::empty()).application_id(app_id)
            .await
            .expect("Error creating client");

        let g_commands = Command::get_global_commands(&client.http).await.unwrap();
        for command in g_commands {
            Command::delete_global_command(&client.http, command.id).await.unwrap();
        }

        let guild_id = GuildId::new(
            dotenv::var("GUILD_ID")
                .expect("Expected GUILD_ID in environment")
                .parse()
                .expect("GUILD_ID must be an integer"),
        );
        let commands = guild_id.get_commands(&client.http).await.unwrap();
        for command in commands {
            guild_id.delete_command(&client.http, command.id).await.unwrap();
        }
        return;
    }

    Ftail::new().daily_file("logs", log::LevelFilter::Debug).init().unwrap();

    let token = dotenv::var("TOKEN").expect("No token string found in .env!");

    let db = Database::connect().await;

    let _ = db.conn.execute("CREATE TABLE IF NOT EXISTS foxholewarbot (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        guild_id INTEGER,
        shard TEXT,
        shard_name TEXT,
        show_command_output INTEGER CHECK (show_command_output IN (0,1))
    )").await.unwrap();

    let intents = GatewayIntents::GUILDS;

    let mut client = Client::builder(token, intents)
        .event_handler(Handler { db })
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
