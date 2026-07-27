mod args;
mod commands;
mod utils;

use args::Args;
use clap::Parser;
use poise::serenity_prelude as serenity;

use utils::cache::save_maps_cache;
use utils::cron::CronHandler;
use utils::db::Database;

/// Shared state, reachable from any command via `ctx.data()`.
pub struct Data {
    pub db: Database,
    pub cron: CronHandler,
    pub local: bool,
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // `tracing::span=off` silences serenity's gateway internals. They arrive as
    // `log` records because `tracing` bridges span lifecycles across, and every
    // heartbeat and received frame produces one — `recv;`, `do_heartbeat;`,
    // `recv_event;`, a few per second, forever, with no content. Serenity's own
    // messages have their own targets and are untouched.
    //
    // `RUST_LOG` still overrides all of this when something needs watching.
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,tracing::span=off"),
    )
    .init();

    let args = Args::parse();

    if args.clear_commands {
        return clear_commands().await;
    }

    let token = dotenv::var("TOKEN").expect("No TOKEN found in .env!");
    let database_url = dotenv::var("DATABASE_URL").expect("No DATABASE_URL found in .env!");

    let db = Database::connect(&database_url).await?;
    let cron = CronHandler::new().await?;
    let local = args.local;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: commands::all(),
            on_error: |error| Box::pin(on_error(error)),
            event_handler: |ctx, event, framework, data| {
                Box::pin(event_handler(ctx, event, framework, data))
            },
            ..Default::default()
        })
        // Runs exactly once per process. The old code kept the "already
        // restarted?" flag in a local it re-created on every `ready`, so every
        // gateway reconnect scheduled a second copy of every job (QA C-6).
        .setup(move |ctx, ready, framework| {
            Box::pin(async move {
                log::info!(
                    "{} is connected, in {} guild(s)",
                    ready.user.name,
                    ready.guilds.len()
                );

                ctx.set_activity(Some(serenity::ActivityData::watching("Foxhole Wars")));
                ctx.idle();

                save_maps_cache().await;
                register_commands(ctx, framework, local).await?;

                // Parsed here so an empty or malformed REVIEWER_IDS is warned
                // about at boot, rather than the first time somebody tries to
                // act on a request nobody can act on.
                utils::review::reviewers();

                cron.start_map_update_job().await?;
                cron.start_request_purge_job(db.clone()).await?;
                cron.restore_jobs(ctx.http.clone(), &db).await;
                // After the restore, so the nightly rebuild is registered
                // against the same jobs it will later replace.
                cron.start_job_rebuild_job(ctx.http.clone(), db.clone())
                    .await?;

                Ok(Data { db, cron, local })
            })
        })
        .build();

    let mut client = serenity::ClientBuilder::new(token, serenity::GatewayIntents::GUILDS)
        .framework(framework)
        .await?;

    client.start().await?;

    Ok(())
}

async fn register_commands(
    ctx: &serenity::Context,
    framework: &poise::Framework<Data, Error>,
    local: bool,
) -> Result<(), Error> {
    let commands = &framework.options().commands;

    if local {
        let guild_id = dev_guild_id()?;
        poise::builtins::register_in_guild(ctx, commands, guild_id).await?;
        log::info!("registered {} commands in the dev guild", commands.len());
    } else {
        poise::builtins::register_globally(ctx, commands).await?;
        log::info!("registered {} commands globally", commands.len());
        clear_dev_guild_commands(ctx).await;
    }

    Ok(())
}

/// Deletes any guild-scoped commands left in the dev guild by a `--local` run.
///
/// Those registrations outlive the process that made them — Discord keeps them
/// until something deletes them — so a dev guild ends up showing two of every
/// command once the real bot is deployed. The duplicate is worse than untidy:
/// a leftover whose name or options no longer match a registered command
/// arrives as an interaction poise has no handler for, so nothing ever
/// acknowledges it and Discord spins until it gives up with "application did
/// not respond".
///
/// Best-effort by design. This runs inside `setup`, where returning an error
/// aborts the whole startup, and failing to tidy the dev guild is never a
/// reason to refuse to run in production.
async fn clear_dev_guild_commands(ctx: &serenity::Context) {
    // No GUILD_ID set means there is no dev guild to clean up, which is the
    // normal case for anyone self-hosting.
    let Ok(guild_id) = dev_guild_id() else {
        return;
    };

    match guild_id
        .set_commands(ctx, Vec::<serenity::CreateCommand>::new())
        .await
    {
        Ok(_) => log::info!("cleared any leftover dev-guild commands in {guild_id}"),
        Err(err) => log::warn!("could not clear dev-guild commands in {guild_id}: {err}"),
    }
}

fn dev_guild_id() -> Result<serenity::GuildId, Error> {
    let raw = dotenv::var("GUILD_ID").map_err(|_| "GUILD_ID is not set in .env")?;
    let id: u64 = raw.parse().map_err(|_| "GUILD_ID must be an integer")?;

    Ok(serenity::GuildId::new(id))
}

/// Deletes every registered command, global and dev-guild.
///
/// Talks to the REST API directly instead of building a gateway client it never
/// connects (QA B-4).
async fn clear_commands() -> Result<(), Error> {
    let token = dotenv::var("TOKEN").expect("No TOKEN found in .env!");
    let app_id: u64 = dotenv::var("APP_ID")
        .expect("No APP_ID found in .env!")
        .parse()
        .map_err(|_| "APP_ID must be an integer")?;

    let http = serenity::Http::new(&token);
    http.set_application_id(serenity::ApplicationId::new(app_id));

    serenity::Command::set_global_commands(&http, Vec::<serenity::CreateCommand>::new()).await?;
    log::info!("cleared global commands");

    match dev_guild_id() {
        Ok(guild_id) => {
            guild_id
                .set_commands(&http, Vec::<serenity::CreateCommand>::new())
                .await?;
            log::info!("cleared commands in guild {guild_id}");
        }
        Err(err) => log::info!("skipping guild commands: {err}"),
    }

    Ok(())
}

async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    _framework: poise::FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> Result<(), Error> {
    match event {
        serenity::FullEvent::GuildCreate { guild, is_new } => {
            if is_new.unwrap_or(false) {
                log::info!(
                    "joined {}, now in {} guild(s)",
                    guild.name,
                    ctx.cache.guilds().len()
                );
            }
        }
        serenity::FullEvent::GuildDelete { incomplete, full } => {
            // `unavailable` means an outage, not a removal — don't delete
            // settings for a guild that's merely offline.
            if incomplete.unavailable {
                return Ok(());
            }

            let name = full
                .as_ref()
                .map(|g| g.name.clone())
                .unwrap_or_else(|| incomplete.id.to_string());

            log::info!("left {name}, now in {} guild(s)", ctx.cache.guilds().len());

            // Cascades to this guild's scheduled reports via the FK.
            if let Err(err) = data.db.delete_guild(incomplete.id.get() as i64).await {
                log::warn!("could not clean up settings for {name}: {err}");
            }
        }
        // Poise routes slash commands and modals for us, but not message
        // components — the Approve/Deny buttons on a review post arrive here.
        serenity::FullEvent::InteractionCreate {
            interaction: serenity::Interaction::Component(component),
        } => {
            // `Ok(false)` means the component wasn't ours, and silence is the
            // right answer — acknowledging an unknown component would claim an
            // interaction something else may own.
            match utils::review::handle_button(ctx, &data.db, component).await {
                Ok(_) => {}
                Err(err) => log::warn!(
                    "could not handle the '{}' button: {err}",
                    component.data.custom_id
                ),
            }
        }
        _ => {}
    }

    Ok(())
}

/// Anything a command returns as an error lands here.
///
/// Always producing a reply is what keeps a deferred interaction from hanging on
/// its spinner forever (QA C-12).
async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    match error {
        poise::FrameworkError::Command { ref error, ctx, .. } => {
            log::error!("command /{} failed: {error}", ctx.command().name);

            let _ = ctx
                .send(
                    poise::CreateReply::default()
                        .content(format!(
                            "Something went wrong running that command. \
                             If it keeps happening, please report it: {}",
                            commands::common::SUPPORT_INVITE
                        ))
                        .ephemeral(true),
                )
                .await;
        }
        poise::FrameworkError::GuildOnly { ctx, .. } => {
            let _ = ctx
                .send(
                    poise::CreateReply::default()
                        .content("This command only works inside a server.")
                        .ephemeral(true),
                )
                .await;
        }
        other => {
            if let Err(err) = poise::builtins::on_error(other).await {
                log::error!("error while handling a framework error: {err}");
            }
        }
    }
}
