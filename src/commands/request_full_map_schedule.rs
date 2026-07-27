//! The applicant side of the full-map scheduling gate
//! (`specs/active/premium-full-map.md`).

use poise::serenity_prelude as serenity;
// The trait, for `RequestModal::execute`. The derive below names its path in
// full, since `Modal` is both a trait and a derive macro.
use poise::Modal as _;

use crate::commands::common::guild_id;
use crate::utils::db::NewFullMapRequest;
use crate::utils::review;
use crate::{Data, Error};

/// Poise needs the application context to open a modal, so this command can't
/// use the usual `Context` alias.
type AppContext<'a> = poise::ApplicationContext<'a, Data, Error>;

/// What the applicant types. Discord allows five inputs, which is exactly what
/// this needs — the channel is a slash option because a modal has no channel
/// picker, and everything else about the server the bot already knows.
#[derive(Debug, poise::Modal)]
#[name = "Schedule a full-map report"]
struct RequestModal {
    #[name = "How often should it post?"]
    #[placeholder = "e.g. every 6 hours, or daily at 18:00 UTC"]
    #[min_length = 3]
    #[max_length = 100]
    cadence: String,

    #[name = "What do you need it for?"]
    #[placeholder = "e.g. regiment ops planning, front-line overview for our members"]
    #[paragraph]
    #[max_length = 600]
    use_case: String,

    #[name = "Roughly who sees it?"]
    #[placeholder = "e.g. ~40 regiment members"]
    #[max_length = 120]
    audience: Option<String>,

    #[name = "Contact handle (optional)"]
    #[placeholder = "Discord handle, if you're happy to be asked follow-up questions"]
    #[max_length = 60]
    contact: Option<String>,

    // Discord modals have no checkbox, so the acknowledgement is a typed word.
    // It is deliberately the last field and deliberately not free-form: the
    // point is that someone read the three conditions, not that they wrote
    // something.
    #[name = "Type YES to confirm you understand"]
    #[placeholder = "Free tool, no uptime guarantee, approval can be revoked"]
    #[min_length = 2]
    #[max_length = 10]
    acknowledgement: String,
}

/// Applies for permission to put a full-map report on a schedule.
//
// One line, because Discord caps a command description at 100 characters and
// poise checks it at compile time. The long-form explanation — and especially
// "no payment is involved" — lives in the reply and in docs/tos.md, which is
// where there is room to say it properly.
#[poise::command(
    slash_command,
    guild_only,
    default_member_permissions = "MANAGE_WEBHOOKS"
)]
pub async fn request_full_map_schedule(
    ctx: AppContext<'_>,
    #[description = "Channel the scheduled full maps would post to."]
    report_channel: serenity::GuildChannel,
) -> Result<(), Error> {
    let base = poise::Context::Application(ctx);
    let guild_id = guild_id(base)?;

    // Same rule as every other guild command: settings first. The request is a
    // step towards a schedule, and a schedule needs a shard.
    let Some(guild) = base.data().db.get_guild(guild_id).await? else {
        base.send(
            poise::CreateReply::default()
                .content(crate::commands::common::NEEDS_SETUP)
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    };

    if guild.full_map_approved {
        base.send(
            poise::CreateReply::default()
                .content(
                    "This server is already approved for scheduled full-map reports — \
                     no need to apply again. Set one up with `/schedule-report`.",
                )
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    // Checked before the modal opens, so nobody fills in five fields only to be
    // told it was pointless. The partial unique index is still the real guard.
    if let Some(open) = base.data().db.pending_request_for_guild(guild.id).await? {
        base.send(
            poise::CreateReply::default()
                .content(format!(
                    "This server already has request **#{}** waiting for review, filed <t:{}:R>. \
                     Only one can be open at a time.",
                    open.id, open.created_at
                ))
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let Some(answers) = RequestModal::execute(ctx).await? else {
        // The applicant closed the modal. Discord has already acknowledged the
        // interaction; there is nothing to reply to and nothing to record.
        return Ok(());
    };

    if !answers.acknowledgement.trim().eq_ignore_ascii_case("yes") {
        base.send(
            poise::CreateReply::default()
                .content(
                    "The last box needs to say `YES` — it's the bit confirming you understand \
                     this is a free tool with no uptime guarantee and that approval can be \
                     withdrawn. Nothing was submitted; run the command again when you're ready.",
                )
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    // Read live and snapshotted onto the row. Review context only — nothing
    // decides anything from it, which is precisely why it doesn't need to be
    // cached anywhere or kept fresh.
    let (guild_name, member_count) = match base.guild() {
        Some(g) => (Some(g.name.clone()), i32::try_from(g.member_count).ok()),
        None => (None, None),
    };

    let request = base
        .data()
        .db
        .create_full_map_request(&NewFullMapRequest {
            guild: guild.id,
            requested_by: base.author().id.get() as i64,
            member_count,
            cadence: answers.cadence.trim().to_string(),
            channel_id: report_channel.id.get() as i64,
            use_case: optional(answers.use_case),
            audience: answers.audience.and_then(optional),
            contact: answers.contact.and_then(optional),
        })
        .await?;

    review::announce(
        &base.serenity_context().http,
        &request,
        guild_name.as_deref(),
    )
    .await;

    base.send(
        poise::CreateReply::default()
            .content(format!(
                "Request **#{}** submitted — thanks. You'll get an answer in <#{}>.\n\n\
                 To be clear about what this is: **no payment is involved and there is no paid \
                 tier.** Rendering the world map on demand with `/full-map` is free for everyone \
                 and always will be. The request exists because a *scheduled* full map stitches \
                 all 53 regions on a timer, forever, whether or not anyone looks at it — this is \
                 a queue, not a price.",
                request.id, report_channel.id
            ))
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

/// Blank-but-present is the same as absent, and storing `"   "` as somebody's
/// use case helps nobody reviewing it.
fn optional(value: String) -> Option<String> {
    let trimmed = value.trim();

    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
