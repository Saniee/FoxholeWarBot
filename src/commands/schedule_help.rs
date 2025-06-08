use serenity::all::{CommandInteraction, Context, CreateCommand, CreateInteractionResponse, CreateInteractionResponseMessage, EditInteractionResponse};

use crate::utils::db::Database;

pub const NAME: &str = "schedule-help";

pub async fn run(ctx: &Context, interaction: &CommandInteraction, db: Database) -> Result<(), serenity::Error> {
    let guild_id: i64 = interaction.guild_id.unwrap().get().try_into().unwrap();
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

    // IMG: https://prnt.sc/JOzbuRouDNmq
    interaction.edit_response(&ctx, EditInteractionResponse::new().content("For further phrases that you can use, refer to this: https://prnt.sc/JOzbuRouDNmq")).await?;

    Ok(())
}

pub fn register() -> CreateCommand {
    CreateCommand::new(NAME).description("Lists all the possible phrases that can be used for the schedule-report command.")
}