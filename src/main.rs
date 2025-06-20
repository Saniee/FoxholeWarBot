use args::Args;
use clap::Parser;

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
use utils::cron::CronHandler;

mod utils;
mod commands;
mod args;

struct Handler {
    db: Database,
    local: bool,
    cron_handler: CronHandler
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = &interaction {
            // println!("Received command interaction!");

            let content = match command.data.name.as_str() {
                commands::get_map::NAME => {
                    commands::get_map::run(&ctx, command, self.db.clone()).await.unwrap();
                    None
                },
                commands::war_report::NAME => {
                    commands::war_report::run(&ctx, command, self.db.clone()).await.unwrap();
                    None
                },
                commands::war_state::NAME => {
                    commands::war_state::run(&ctx, command, self.db.clone()).await.unwrap();
                    None
                },
                commands::set_guild_settings::NAME => {
                    commands::set_guild_settings::run(&ctx, command, self.db.clone()).await.unwrap();
                    None
                },
                commands::schedule_report::NAME => {
                    commands::schedule_report::run(&ctx, command, self.db.clone(), &mut self.cron_handler.clone()).await.unwrap();
                    None
                },
                commands::remove_report::NAME => {
                    commands::remove_report::run(&ctx, command, self.db.clone(), &mut self.cron_handler.clone()).await.unwrap();
                    None
                },
                commands::schedule_help::NAME => {
                    commands::schedule_help::run(&ctx, command, self.db.clone()).await.unwrap();
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
                commands::get_map::NAME => {
                    commands::get_map::autocomplete(&ctx, command, self.db.clone()).await.unwrap();
                },
                commands::war_report::NAME => {
                    commands::war_report::autocomplete(&ctx, command, self.db.clone()).await.unwrap();
                },
                commands::set_guild_settings::NAME => {
                    commands::set_guild_settings::autocomplete(&ctx, command).await.unwrap();
                },
                commands::schedule_report::NAME => {
                    commands::schedule_report::autocomplete(&ctx, command, self.db.clone()).await.unwrap();
                },
                commands::remove_report::NAME => {
                    commands::remove_report::autocomplete(&ctx, command, self.db.clone()).await.unwrap();
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
            self.db.create_guild(i64::try_from(guild.id.get()).unwrap(), None).await;
        }
    }

    async fn guild_delete(&self, _ctx: Context, incomplete: UnavailableGuild, full: Option<Guild>) {
        if full.is_some() {
            if !incomplete.unavailable {
                println!("Left {}. Now in {} Guilds!", full.clone().unwrap().name, _ctx.cache.guilds().len());
                self.db.delete_guild(i64::try_from(full.clone().unwrap().id.get()).unwrap()).await;
            }
        } else if !incomplete.unavailable {
            println!("Left {} (Incomplete Data). Now in {} Guilds!", incomplete.id, _ctx.cache.guilds().len());
            self.db.delete_guild(i64::try_from(incomplete.id.get()).unwrap()).await;
        }
    }
    
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!\nIn {} guild/s!", ready.user.name, ready.guilds.len());

        ctx.set_activity(Some(ActivityData::watching("Foxhole Wars")));
        ctx.idle();

        save_maps_cache().await;
        self.cron_handler.start_map_update_job().await;

        let guild_id = GuildId::new(
            dotenv::var("GUILD_ID")
                .expect("Expected GUILD_ID in environment")
                .parse()
                .expect("GUILD_ID must be an integer"),
        );

        if !&self.local {
            let _ = Command::set_global_commands(&ctx.http, vec![
                commands::get_map::register(),
                commands::war_report::register(),
                commands::war_state::register(),
                commands::schedule_report::register(),
                commands::schedule_help::register(),
                commands::remove_report::register(),
                commands::set_guild_settings::register(),
            ]).await.unwrap();
        } else {
            let _ = guild_id.set_commands(&ctx.http, vec![
                commands::get_map::register(),
                commands::war_report::register(),
                commands::war_state::register(),
                commands::schedule_report::register(),
                commands::schedule_help::register(),
                commands::remove_report::register(),
                commands::set_guild_settings::register(),
            ])
            .await.unwrap();
        }

        self.cron_handler.clone().restart_report_jobs(&ctx, self.db.clone()).await;
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let local = args.local;
    let cron_handler = CronHandler::new().await.unwrap();

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

    let token = dotenv::var("TOKEN").expect("No token string found in .env!");

    let db = Database::connect().await;

    let _ = db.conn.execute("
        CREATE TABLE IF NOT EXISTS guilds (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        guild_id INTEGER,
        shard TEXT,
        shard_name TEXT,
        show_command_output INTEGER CHECK (show_command_output IN (0,1)) )").await.unwrap();

    let _ = db.conn.execute("
        CREATE TABLE IF NOT EXISTS cronjobs (
        guild INTEGER REFERENCES guilds(id),
        job_name TEXT UNIQUE,
        schedule TEXT,
        webhook_url TEXT,
        map_name TEXT,
        draw_text INTEGER CHECK (draw_text IN (0,1)),
        job_id TEXT UNIQUE )").await.unwrap();

    let intents = GatewayIntents::GUILDS;

    db.migrate().await;

    let mut client = Client::builder(token, intents)
        .event_handler(Handler { db, local, cron_handler })
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
