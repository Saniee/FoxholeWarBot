use serenity::all::{AutocompleteChoice, CommandInteraction, Context, CreateAutocompleteResponse, CreateCommand, CreateCommandOption, CreateInteractionResponse, CreateInteractionResponseMessage, CreateWebhook, EditInteractionResponse, Webhook};

use crate::utils::{cache::load_maps, cron::{CronHandler}, db::{Database, Shard}};

#[derive(Clone)]
pub struct ReportJob {
    pub schedule_name: String,
    pub schedule: String,
    pub webhook: Webhook,
    pub guild_id: i64,
    pub map_name: String,
    pub draw_text: i32,
    pub db: Database,
}

pub const NAME: &str = "schedule-report";

pub async fn run(ctx: &Context, interaction: &CommandInteraction, db: Database, cron_handler: &mut CronHandler) -> Result<(), serenity::Error> {
    let guild_id: i64 = interaction.guild_id.unwrap().get().try_into().unwrap();
    let data = db.get_guild(guild_id).await;

    let guild = match data {
        None => {
            interaction.create_response(ctx, CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().ephemeral(true).content("No Shard set for the guild. Run the `/set-guild-settings` command to do so."))).await?;
            return Ok(());
        }
        Some(data) => data
    };
    let guild_id = guild.guild_id;

    if guild.show_command_output == 1 {
        interaction.defer(ctx).await?;
    } else if guild.show_command_output == 0 {
        interaction.defer_ephemeral(ctx).await?;
    }

    let map_name = interaction.data.options[0].value.as_str().unwrap().to_owned();
    let schedule_name = interaction.data.options[1].value.as_str().unwrap().to_owned();
    let channel = interaction.data.options[2].value.as_channel_id().unwrap();
    let schedule = interaction.data.options[3].value.as_str().unwrap().to_owned();
    
    
    let draw_text_bool = interaction.data.options[4].value.as_bool().unwrap();
    #[allow(clippy::needless_late_init)]
    let draw_text;

    if draw_text_bool {
        draw_text = 1;
    } else {
        draw_text = 0;
    }

    let webhooks = channel.webhooks(ctx).await.unwrap();
    let webhook_find = webhooks.iter().find(|webhook| webhook.name == Some("Scheduled Map Report Webhook".to_string()));
    let webhook: Webhook;

    if webhook_find.is_none() {
        let create_webhook = channel.create_webhook(ctx, CreateWebhook::new("Scheduled Map Report Webhook")).await;
        webhook = match create_webhook {
            Ok(w) => w,
            Err(e) => {
                interaction.edit_response(&ctx, EditInteractionResponse::new().content(format!("Error! {e}"))).await?;
                return Ok(())
            }
        };
    } else {
        webhook = webhook_find.unwrap().clone();
    }

    cron_handler.add_report_job(ctx.clone(), db.clone(), ReportJob { schedule_name: schedule_name.clone(), schedule, webhook, db, guild_id, map_name, draw_text }, false).await;
    
    interaction.edit_response(ctx, EditInteractionResponse::new().content(format!("Your scheduled report with the name: {}, was created! You can look into <#{}> for it working.", schedule_name, channel.get()))).await?;

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
    CreateCommand::new(NAME).description("wip")
    .add_option(CreateCommandOption::new(serenity::all::CommandOptionType::String, "map-name", "wip").required(true).set_autocomplete(true))
    .add_option(CreateCommandOption::new(serenity::all::CommandOptionType::String, "schedule-name", "wip").required(true))
    .add_option(CreateCommandOption::new(serenity::all::CommandOptionType::Channel, "report-channel", "The channel where the bot will send the reports.").required(true))
    .add_option(CreateCommandOption::new(serenity::all::CommandOptionType::String, "schedule", "wip").required(true))
    .add_option(CreateCommandOption::new(serenity::all::CommandOptionType::Boolean, "draw-text", "wip").required(true))
}
