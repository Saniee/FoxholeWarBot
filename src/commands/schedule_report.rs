use serenity::all::{CommandInteraction, Context, CreateCommand};

use crate::utils::{cron::CronHandler, db::Database};

pub const NAME: &str = "schedule-report";

pub async fn run(ctx: &Context, interaction: &CommandInteraction, db: Database, cron_handler: CronHandler) -> Result<(), serenity::Error> {
    
    
    Ok(())
}

pub fn register() -> CreateCommand {
    CreateCommand::new(NAME)
}