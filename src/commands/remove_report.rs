use serenity::all::{AutocompleteChoice, CommandInteraction, Context, CreateAutocompleteResponse, CreateCommand, CreateCommandOption, CreateInteractionResponse, CreateInteractionResponseMessage, EditInteractionResponse};

use crate::utils::{cron::{CronError, CronHandler}, db::Database};

pub const NAME: &str = "remove-report";

pub async fn run(ctx: &Context, interaction: &CommandInteraction, db: Database, cron_handler: &mut CronHandler) -> Result<(), serenity::Error> {
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

    let schedule_name = interaction.data.options[0].value.as_str().unwrap();

    match cron_handler.remove_report_job(ctx, db.clone(), schedule_name.to_string(), guild.clone()).await {
        Ok(_) => {
            interaction.edit_response(ctx, EditInteractionResponse::new().content(format!("Your scheduled report with the name: {}, was removed!", schedule_name))).await?;
        },
        Err(err) => {
            match err {
                CronError::NoJobFound(_) => {
                    interaction.edit_response(ctx, EditInteractionResponse::new().content("The job name you have inputed is not in the guild database. Please double check if you typed it out correctly.".to_string())).await?;
                },
                CronError::JobListEmpty(_) => {
                    interaction.edit_response(ctx, EditInteractionResponse::new().content("Your guild currently has no scheduled reports. Cannot remove nothing.".to_string())).await?;
                },
                _ => {
                    interaction.edit_response(ctx, EditInteractionResponse::new().content("An unknown error occured! Please report it to the support server: [Invite](https://discord.com/invite/9wzppSgXdQ)".to_string())).await?;
                }
            }
        }
    }

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

    let jobs = db.get_jobs_for_guild(guild).await;

    let filter = interaction.data.options[0].value.as_str().unwrap_or("").trim().to_lowercase();

    if jobs.is_empty() {
        choices.push(AutocompleteChoice::new("There are no scheduled reports for this guild.", "-"));
    } else {
        for job in jobs {
        if job.job_name.trim().contains(&filter) && choices.len() < 25 {
            choices.push(AutocompleteChoice::new(job.clone().job_name, job.job_name));
        }
    }
    }

    let response = CreateInteractionResponse::Autocomplete(CreateAutocompleteResponse::new().set_choices(choices));

    interaction.create_response(&ctx.http, response).await?;
    Ok(())
}

pub fn register() -> CreateCommand {
    CreateCommand::new(NAME).description("Removes a scheduled report.").add_option(CreateCommandOption::new(serenity::all::CommandOptionType::String, "schedule-name", "The name of the schedule which will be deleted.").required(true).set_autocomplete(true))
}