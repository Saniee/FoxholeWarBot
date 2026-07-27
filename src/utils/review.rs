//! The review surface for full-map schedule requests: the embed a reviewer sees,
//! the Approve/Deny buttons, and who is allowed to press them.
//!
//! Both review paths land here. The channel post is the primary surface and
//! `/full-map-requests` is the fallback, but they drive the same
//! `review_full_map_request` call and render the same embed, so they cannot
//! diverge (`specs/active/premium-full-map.md`).

use std::sync::OnceLock;

use poise::serenity_prelude as serenity;

use super::db::{Database, FullMapRequest, RequestStatus};

/// Prefix on every button this module owns, so the interaction handler can tell
/// its own components from anything else the bot ever adds.
const CUSTOM_ID_PREFIX: &str = "fullmap-request";

/// The support-server channel where requests are posted for review. Optional:
/// unset means command-only review, which is a degraded surface, never a
/// dropped request.
fn requests_channel() -> Option<serenity::ChannelId> {
    static CHANNEL: OnceLock<Option<serenity::ChannelId>> = OnceLock::new();

    *CHANNEL.get_or_init(|| match dotenv::var("REQUESTS_CHANNEL_ID") {
        Ok(raw) => match raw.trim().parse::<u64>() {
            Ok(id) => Some(serenity::ChannelId::new(id)),
            Err(_) => {
                log::warn!("REQUESTS_CHANNEL_ID is not an integer, falling back to command review");
                None
            }
        },
        Err(_) => None,
    })
}

/// What a reviewer needs to decide, and nothing they don't.
pub fn request_embed(request: &FullMapRequest, guild_name: Option<&str>) -> serenity::CreateEmbed {
    let title = match guild_name {
        Some(name) => format!("Full-map schedule request #{} — {name}", request.id),
        None => format!("Full-map schedule request #{}", request.id),
    };

    let color = match request.status.as_str() {
        "approved" => (60, 180, 75),
        "denied" => (200, 60, 60),
        _ => (250, 190, 60),
    };

    let mut embed = serenity::CreateEmbed::new()
        .title(title)
        .color(color)
        .field("Server", format!("`{}`", request.guild_id), true)
        .field(
            "Requested by",
            format!("<@{}>", request.requested_by),
            true,
        )
        // Context, not an input. Stated as such so a reviewer doesn't start
        // treating it as a threshold the bot is quietly applying.
        .field(
            "Members (context only)",
            request
                .member_count
                .map_or_else(|| "unknown".to_string(), |n| n.to_string()),
            true,
        )
        .field("Requested cadence", request.cadence.as_str(), false)
        .field("Posting to", format!("<#{}>", request.channel_id), true)
        .field("Status", request.status.as_str(), true)
        // Discord renders this in each reviewer's own timezone.
        .field("Filed", format!("<t:{}:R>", request.created_at), true);

    for (name, value) in [
        ("Use case", request.use_case.as_deref()),
        ("Expected audience", request.audience.as_deref()),
        ("Contact", request.contact.as_deref()),
    ] {
        if let Some(value) = value.map(str::trim).filter(|v| !v.is_empty()) {
            embed = embed.field(name, value, false);
        }
    }

    if let Some(reviewer) = request.reviewed_by {
        embed = embed.field("Decided by", format!("<@{reviewer}>"), false);
    }

    embed
}

/// Approve/Deny, or nothing at all once the request has been decided — a
/// settled request keeps its post as a record, but the buttons stop being a way
/// to overturn it by accident.
pub fn review_buttons(request: &FullMapRequest) -> Vec<serenity::CreateActionRow> {
    if !request.is_pending() {
        return Vec::new();
    }

    vec![serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new(format!("{CUSTOM_ID_PREFIX}:approve:{}", request.id))
            .label("Approve")
            .style(serenity::ButtonStyle::Success),
        serenity::CreateButton::new(format!("{CUSTOM_ID_PREFIX}:deny:{}", request.id))
            .label("Deny")
            .style(serenity::ButtonStyle::Danger),
    ])]
}

/// Posts a new request to the review channel.
///
/// Every failure here is logged and swallowed. The request is already committed
/// by the time this runs, and losing the notification is a reviewer checking
/// `/full-map-requests` instead — losing the request because a channel id was
/// wrong would be unforgivable.
pub async fn announce(http: &serenity::Http, request: &FullMapRequest, guild_name: Option<&str>) {
    let Some(channel) = requests_channel() else {
        log::info!(
            "REQUESTS_CHANNEL_ID is unset — request #{} is reviewable with /full-map-requests only",
            request.id
        );
        return;
    };

    let message = serenity::CreateMessage::new()
        .embed(request_embed(request, guild_name))
        .components(review_buttons(request));

    match channel.send_message(http, message).await {
        Ok(_) => log::info!("posted request #{} for review", request.id),
        Err(err) => log::warn!(
            "could not post request #{} to the review channel: {err} \
             — it is still pending and visible to /full-map-requests",
            request.id
        ),
    }
}

/// May this user decide requests?
///
/// The application's owner, or any member of its team. Nothing else — and
/// specifically **not** "an administrator of the server this was invoked in".
/// A reviewer sees other servers' free-text answers and can grant recurring load
/// on the host, so the gate has to be the people who own the bot, not the people
/// who own a server that happens to have added it.
pub async fn is_reviewer(ctx: &serenity::Context, user: serenity::UserId) -> bool {
    let Ok(info) = ctx.http.get_current_application_info().await else {
        // Failing closed is right: an unreachable Discord must not hand the
        // approve button to whoever pressed it.
        log::warn!("could not read application info, refusing the review action");
        return false;
    };

    if info.owner.as_ref().is_some_and(|owner| owner.id == user) {
        return true;
    }

    info.team
        .is_some_and(|team| team.members.iter().any(|m| m.user.id == user))
}

/// Handles an Approve/Deny press. `Ok(false)` means the component wasn't ours.
pub async fn handle_button(
    ctx: &serenity::Context,
    db: &Database,
    interaction: &serenity::ComponentInteraction,
) -> Result<bool, serenity::Error> {
    let Some(rest) = interaction.data.custom_id.strip_prefix(CUSTOM_ID_PREFIX) else {
        return Ok(false);
    };

    let mut parts = rest.trim_start_matches(':').split(':');

    let status = match parts.next() {
        Some("approve") => RequestStatus::Approved,
        Some("deny") => RequestStatus::Denied,
        _ => return Ok(false),
    };

    let Some(id) = parts.next().and_then(|id| id.parse::<i64>().ok()) else {
        return Ok(false);
    };

    if !is_reviewer(ctx, interaction.user.id).await {
        reply(ctx, interaction, "Only a reviewer can decide these requests.").await?;
        return Ok(true);
    }

    let decided = match db
        .review_full_map_request(id, status, interaction.user.id.get() as i64)
        .await
    {
        Ok(decided) => decided,
        Err(err) => {
            log::warn!("could not record a decision on request #{id}: {err}");
            reply(ctx, interaction, "Couldn't record that. Nothing changed.").await?;
            return Ok(true);
        }
    };

    // `review_full_map_request` only updates a row that is still pending, so
    // `None` here is a second reviewer arriving after the first — not an error,
    // and specifically not a decision to overturn.
    let Some(request) = decided else {
        reply(
            ctx,
            interaction,
            "That request has already been decided by someone else.",
        )
        .await?;
        return Ok(true);
    };

    // Edit the original post rather than replying under it: the message is the
    // record of the request, and a stale "pending" embed with dead buttons under
    // it is how two reviewers end up disagreeing about what happened.
    interaction
        .create_response(
            ctx,
            serenity::CreateInteractionResponse::UpdateMessage(
                serenity::CreateInteractionResponseMessage::new()
                    .embed(request_embed(&request, None))
                    .components(review_buttons(&request)),
            ),
        )
        .await?;

    notify_requester(ctx, &request).await;

    Ok(true)
}

/// Tells the requesting server what was decided, in the channel they nominated
/// for the reports.
///
/// Not a DM. `docs/privacy.md` states the bot never sends one, and that promise
/// is worth more than the convenience — the channel they picked is somewhere
/// they are already expecting this bot to speak.
pub async fn notify_requester(ctx: &serenity::Context, request: &FullMapRequest) {
    let approved = request.status == RequestStatus::Approved.as_str();

    let body = if approved {
        format!(
            "<@{}> — your request to schedule full-map reports here was **approved**. \
             Set it up with `/schedule-report`.",
            request.requested_by
        )
    } else {
        format!(
            "<@{}> — your request to schedule full-map reports here wasn't approved this time. \
             `/full-map` still renders the world map on demand, for free, as often as you like.",
            request.requested_by
        )
    };

    let channel = serenity::ChannelId::new(request.channel_id as u64);

    if let Err(err) = channel
        .send_message(ctx, serenity::CreateMessage::new().content(body))
        .await
    {
        log::warn!(
            "decided request #{} but could not tell <#{}>: {err}",
            request.id,
            request.channel_id
        );
    }
}

async fn reply(
    ctx: &serenity::Context,
    interaction: &serenity::ComponentInteraction,
    content: &str,
) -> Result<(), serenity::Error> {
    interaction
        .create_response(
            ctx,
            serenity::CreateInteractionResponse::Message(
                serenity::CreateInteractionResponseMessage::new()
                    .content(content)
                    .ephemeral(true),
            ),
        )
        .await
}
