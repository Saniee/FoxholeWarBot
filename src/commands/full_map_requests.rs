//! The reviewer side of the full-map scheduling gate: the fallback for when the
//! requests channel isn't configured, isn't reachable, or the buttons on an old
//! post have scrolled out of reach.
//!
//! Deliberately the *same* rows and the same `review_full_map_request` call the
//! buttons use (`utils::review`), so the two surfaces can't disagree about what
//! was decided.

use crate::utils::db::RequestStatus;
use crate::utils::review;
use crate::{Context, Error};

/// Reviews pending full-map schedule requests.
#[poise::command(
    slash_command,
    subcommands("list", "approve", "deny", "revoke"),
    subcommand_required,
    // Hides it from most members. The real gate is `is_reviewer` — this only
    // keeps the command out of the picker for people it would refuse anyway.
    default_member_permissions = "ADMINISTRATOR"
)]
pub async fn full_map_requests(_: Context<'_>) -> Result<(), Error> {
    // `subcommand_required` means this body is unreachable, but poise still
    // wants the parent to exist.
    Ok(())
}

/// Shows the full-map schedule requests still waiting for a decision.
#[poise::command(slash_command)]
async fn list(ctx: Context<'_>) -> Result<(), Error> {
    if !reviewer(ctx).await? {
        return Ok(());
    }

    let pending = ctx.data().db.pending_full_map_requests().await?;

    if pending.is_empty() {
        ctx.say("Nothing waiting for review.").await?;
        return Ok(());
    }

    // One embed each rather than a summary table: the free-text answers are the
    // whole reason to look, and a table would truncate exactly them. Discord
    // caps a message at 10 embeds; more than that pending is its own problem.
    let mut reply = poise::CreateReply::default().ephemeral(true).content(
        format!("{} request(s) waiting.", pending.len()),
    );

    for request in pending.iter().take(10) {
        reply = reply.embed(review::request_embed(request, None));
    }

    ctx.send(reply).await?;

    Ok(())
}

/// Approves a request, letting that server schedule full-map reports.
#[poise::command(slash_command)]
async fn approve(
    ctx: Context<'_>,
    #[description = "Request id, from /full-map-requests list."] id: i64,
) -> Result<(), Error> {
    decide(ctx, id, RequestStatus::Approved).await
}

/// Denies a request. The server keeps on-demand /full-map either way.
#[poise::command(slash_command)]
async fn deny(
    ctx: Context<'_>,
    #[description = "Request id, from /full-map-requests list."] id: i64,
) -> Result<(), Error> {
    decide(ctx, id, RequestStatus::Denied).await
}

/// Withdraws a server's approval. Its schedules pause, they aren't deleted.
#[poise::command(slash_command)]
async fn revoke(
    ctx: Context<'_>,
    #[description = "The server's Discord id."] guild_id: String,
) -> Result<(), Error> {
    if !reviewer(ctx).await? {
        return Ok(());
    }

    let Ok(parsed) = guild_id.trim().parse::<i64>() else {
        ctx.say("That isn't a server id — it should be a long number.")
            .await?;
        return Ok(());
    };

    let Some(guild) = ctx.data().db.get_guild(parsed).await? else {
        ctx.say("No server with that id has ever set this bot up.")
            .await?;
        return Ok(());
    };

    // Same call the Revoke button makes, so the guild flag and the request that
    // granted it can't end up disagreeing about whether the approval still
    // stands. `false` means it wasn't approved in the first place — the
    // statement's own `WHERE`, not a check either surface has to remember.
    let revoked = ctx
        .data()
        .db
        .revoke_full_map_approval(guild.id, ctx.author().id.get() as i64)
        .await?;

    if !revoked {
        ctx.say("That server isn't approved, so there's nothing to withdraw.")
            .await?;
        return Ok(());
    }

    ctx.say(format!(
        "Approval withdrawn for `{parsed}`. Its full-map schedules pause at the next tick — \
         paused, not deleted."
    ))
    .await?;

    Ok(())
}

async fn decide(ctx: Context<'_>, id: i64, status: RequestStatus) -> Result<(), Error> {
    if !reviewer(ctx).await? {
        return Ok(());
    }

    let decided = ctx
        .data()
        .db
        .review_full_map_request(id, status, ctx.author().id.get() as i64)
        .await?;

    // Same statement the buttons run, so the same "already decided" answer comes
    // back rather than a second decision landing on top of the first.
    let Some(request) = decided else {
        let known = ctx.data().db.get_full_map_request(id).await?;

        let message = match known {
            Some(request) => format!(
                "Request #{id} was already **{}**. Nothing changed.",
                request.status
            ),
            None => format!("No request #{id}."),
        };

        ctx.say(message).await?;
        return Ok(());
    };

    review::notify_requester(ctx.serenity_context(), &request).await;

    ctx.send(
        poise::CreateReply::default()
            .ephemeral(true)
            .content(format!("Request #{id} {}.", request.status))
            .embed(review::request_embed(&request, None)),
    )
    .await?;

    Ok(())
}

/// Ephemeral by default and reviewer-gated: these replies carry other servers'
/// free-text answers, which belong to whoever is reviewing them and nobody else
/// in the channel.
async fn reviewer(ctx: Context<'_>) -> Result<bool, Error> {
    ctx.defer_ephemeral().await?;

    let allowed = review::is_reviewer(ctx.author().id);

    if !allowed {
        ctx.say("Only someone in the bot's `REVIEWER_IDS` can review these.")
            .await?;
    }

    Ok(allowed)
}
