use serenity::all::{AutocompleteChoice, CommandInteraction, Context, CreateAutocompleteResponse, CreateCommand, CreateCommandOption, CreateInteractionResponse};

use crate::utils::{cron::CronHandler, db::Database};

pub const NAME: &str = "remove-report";

pub async fn run(ctx: &Context, interaction: &CommandInteraction, db: Database, cron_handler: &mut CronHandler) -> Result<(), serenity::Error> {
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

    for job in jobs {
        if job.job_name.trim().contains(&filter) && choices.len() < 25 {
            choices.push(AutocompleteChoice::new(job.clone().job_name, job.job_name));
        }
    }

    let response = CreateInteractionResponse::Autocomplete(CreateAutocompleteResponse::new().set_choices(choices));

    interaction.create_response(&ctx.http, response).await?;
    Ok(())
}

pub fn register() -> CreateCommand {
    CreateCommand::new(NAME).description("wip").add_option(CreateCommandOption::new(serenity::all::CommandOptionType::String, "schedule-name", "wip").required(true).set_autocomplete(true))
}