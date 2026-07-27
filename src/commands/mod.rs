pub mod common;
pub mod full_map;
pub mod get_map;
pub mod remove_report;
pub mod schedule_help;
pub mod schedule_report;
pub mod set_guild_settings;
pub mod war_report;
pub mod war_state;

use crate::{Data, Error};

/// Every slash command the bot registers. One list, used for both global and
/// guild-scoped registration.
pub fn all() -> Vec<poise::Command<Data, Error>> {
    vec![
        get_map::get_map(),
        full_map::full_map(),
        war_report::war_report(),
        war_state::war_state(),
        set_guild_settings::set_guild_settings(),
        schedule_help::schedule_help(),
        schedule_report::schedule_report(),
        remove_report::remove_report(),
    ]
}
